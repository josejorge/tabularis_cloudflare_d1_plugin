use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::PluginError;

// ---------------------------------------------------------------------------
// D1 API response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct D1ListResponse {
    pub result: Vec<D1DatabaseInfo>,
    pub success: bool,
    #[serde(default)]
    pub errors: Vec<D1ApiError>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct D1DatabaseInfo {
    pub uuid: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct D1QueryResponse {
    pub result: Vec<D1ResultSet>,
    pub success: bool,
    #[serde(default)]
    pub errors: Vec<D1ApiError>,
}

#[derive(Debug, Deserialize)]
pub struct D1ResultSet {
    #[serde(default)]
    pub results: Vec<serde_json::Map<String, Value>>,
    #[allow(dead_code)]
    pub success: bool,
    pub meta: D1Meta,
}

#[derive(Debug, Deserialize, Default)]
pub struct D1Meta {
    #[allow(dead_code)]
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub changes: i64,
    #[allow(dead_code)]
    #[serde(default)]
    pub last_row_id: i64,
    #[allow(dead_code)]
    #[serde(default)]
    pub rows_read: i64,
    #[allow(dead_code)]
    #[serde(default)]
    pub rows_written: i64,
}

#[derive(Debug, Deserialize)]
pub struct D1ApiError {
    #[allow(dead_code)]
    pub code: u32,
    pub message: String,
}

// ---------------------------------------------------------------------------
// In-process cache: database name → UUID
// Populated on get_databases; refreshed on cache miss.
// ---------------------------------------------------------------------------

static DB_ID_CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn cache_databases(databases: &[D1DatabaseInfo]) {
    let mut cache = DB_ID_CACHE.lock().expect("cache lock poisoned");
    for db in databases {
        cache.insert(db.name.clone(), db.uuid.clone());
    }
}

pub fn cached_db_id(name: &str) -> Option<String> {
    DB_ID_CACHE
        .lock()
        .expect("cache lock poisoned")
        .get(name)
        .cloned()
}

// ---------------------------------------------------------------------------
// D1Client
// ---------------------------------------------------------------------------

pub struct D1Client {
    http: Client,
    api_token: String,
    account_id: String,
}

impl D1Client {
    pub fn new(api_token: &str, account_id: &str) -> Result<Self, PluginError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent(concat!(
                "tabularis-cloudflare-d1-plugin/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(|e| PluginError::Client(e.to_string()))?;
        Ok(Self {
            http,
            api_token: api_token.to_string(),
            account_id: account_id.to_string(),
        })
    }

    fn db_base(&self) -> String {
        format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/d1/database",
            self.account_id
        )
    }

    /// List all D1 databases for this account.
    pub fn list_databases(&self) -> Result<Vec<D1DatabaseInfo>, PluginError> {
        let resp = self
            .http
            .get(&self.db_base())
            .bearer_auth(&self.api_token)
            .send()
            .map_err(|e| PluginError::Http(e.to_string()))?;

        let status = resp.status();
        if status == 401 || status == 403 {
            return Err(PluginError::Api(
                "Authentication failed — check your API Token".into(),
            ));
        }
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(PluginError::Api(format!("HTTP {}: {}", status, body)));
        }

        let parsed: D1ListResponse =
            resp.json().map_err(|e| PluginError::Parse(e.to_string()))?;
        if !parsed.success {
            let msg = parsed
                .errors
                .first()
                .map(|e| e.message.as_str())
                .unwrap_or("unknown D1 error");
            return Err(PluginError::Api(msg.to_string()));
        }
        Ok(parsed.result)
    }

    /// Resolve a database name or UUID to a UUID, refreshing the cache if
    /// needed.
    pub fn resolve_db_id(&self, name_or_id: &str) -> Result<String, PluginError> {
        if is_uuid(name_or_id) {
            return Ok(name_or_id.to_string());
        }

        if let Some(id) = cached_db_id(name_or_id) {
            return Ok(id);
        }

        // Cache miss — refresh.
        let dbs = self.list_databases()?;
        cache_databases(&dbs);

        cached_db_id(name_or_id).ok_or_else(|| {
            PluginError::Api(format!(
                "D1 database '{}' not found in account '{}'",
                name_or_id, self.account_id
            ))
        })
    }

    /// Execute a single SQL statement against a D1 database (by UUID).
    pub fn query(
        &self,
        db_id: &str,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<D1ResultSet, PluginError> {
        let url = format!("{}/{}/query", self.db_base(), db_id);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.api_token)
            .json(&json!({ "sql": sql, "params": params }))
            .send()
            .map_err(|e| PluginError::Http(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(PluginError::Api(format!("HTTP {}: {}", status, body)));
        }

        let parsed: D1QueryResponse =
            resp.json().map_err(|e| PluginError::Parse(e.to_string()))?;
        check_response_errors(&parsed.success, &parsed.errors)?;

        parsed
            .result
            .into_iter()
            .next()
            .ok_or_else(|| PluginError::Api("D1 returned empty result array".into()))
    }

    /// Execute multiple SQL statements sequentially, returning one D1ResultSet
    /// per statement in order.
    ///
    /// The D1 HTTP REST API has no /batch endpoint (batch is Workers-binding-only),
    /// so this runs each statement as a separate /query call.
    pub fn batch(
        &self,
        db_id: &str,
        requests: &[(&str, Vec<Value>)],
    ) -> Result<Vec<D1ResultSet>, PluginError> {
        let mut results = Vec::with_capacity(requests.len());
        for (sql, params) in requests {
            results.push(self.query(db_id, sql, params.clone())?);
        }
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn check_response_errors(success: &bool, errors: &[D1ApiError]) -> Result<(), PluginError> {
    if !success {
        let msg = errors
            .first()
            .map(|e| e.message.as_str())
            .unwrap_or("unknown D1 error");
        return Err(PluginError::Api(msg.to_string()));
    }
    Ok(())
}

/// Returns true if `s` looks like a UUID (8-4-4-4-12 hex groups).
pub fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let expected = [8usize, 4, 4, 4, 12];
    parts.iter().zip(expected.iter()).all(|(p, &len)| {
        p.len() == len && p.chars().all(|c| c.is_ascii_hexdigit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_detection() {
        assert!(is_uuid("01234567-89ab-cdef-0123-456789abcdef"));
        assert!(!is_uuid("my-database"));
        assert!(!is_uuid("production"));
        assert!(!is_uuid("01234567-89ab-cdef-0123")); // too short
    }
}
