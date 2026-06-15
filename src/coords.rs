use serde_json::Value;

use crate::error::PluginError;

/// Connection-level credentials and target database, extracted from every RPC
/// params["params"] object. Uses the standard Tabularis connection fields:
///   username → Cloudflare Account ID
///   password → Cloudflare API Bearer token
///   database → D1 database name (resolved to UUID via cache)
pub struct ConnectionCoords {
    pub account_id: String,
    pub api_token: String,
    pub database: String,
}

impl ConnectionCoords {
    pub fn from_params(params: &Value) -> Result<Self, PluginError> {
        let p = &params["params"];

        let account_id = p["username"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| PluginError::Config("Account ID is required (Username field)".into()))?
            .to_string();

        let api_token = p["password"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                PluginError::Config("API Token is required (Password field)".into())
            })?
            .to_string();

        // Tabularis passes the sidebar-selected database as params["schema"].
        // Fall back to the connection-configured params["params"]["database"]
        // for handlers that don't receive a schema (e.g. execute_query run from
        // the query editor without a sidebar selection).
        let database = match params["schema"].as_str().filter(|s| !s.is_empty()) {
            Some(s) => s.trim().to_string(),
            None => match &p["database"] {
                Value::String(s) => s.trim().to_string(),
                Value::Array(arr) => arr
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                _ => String::new(),
            },
        };

        Ok(Self {
            account_id,
            api_token,
            database,
        })
    }

/// Like from_params but does not require the database field to be set.
    /// Used for get_databases / test_connection where we only need credentials.
    pub fn credentials_only(params: &Value) -> Result<(String, String), PluginError> {
        let p = &params["params"];
        let account_id = p["username"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| PluginError::Config("Account ID is required (Username field)".into()))?
            .to_string();
        let api_token = p["password"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                PluginError::Config("API Token is required (Password field)".into())
            })?
            .to_string();
        Ok((account_id, api_token))
    }
}
