// File: coords.rs
// Description: Extracts Cloudflare credentials and the target database from RPC params, with an optional plugin-level default Account ID.
// Author: Jose-Jorge HERNANDEZ
// Company: Parlee Conseiller, Inc.
// Date: 2026-06-15
// Last edit date: 2026-10-10
// Version: 1.1.0

use std::sync::{LazyLock, Mutex};

use serde_json::Value;

use crate::error::PluginError;

/// Settings key (declared in the manifest) holding the fallback Account ID.
pub const SETTING_DEFAULT_ACCOUNT_ID: &str = "default_account_id";

/// Fallback Account ID from the plugin settings, filled in by `initialize`.
/// Only the Account ID is stored here (it is not a secret); the API token
/// stays in the connection's Password field.
static DEFAULT_ACCOUNT_ID: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

/// Stores the settings sent by the host in the `initialize` call.
pub fn apply_settings(settings: &Value) {
    let id = settings[SETTING_DEFAULT_ACCOUNT_ID]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if let Ok(mut guard) = DEFAULT_ACCOUNT_ID.lock() {
        *guard = id;
    }
}

fn default_account_id() -> String {
    DEFAULT_ACCOUNT_ID
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

/// Picks the connection's Username when set, otherwise the plugin default.
/// Returns an error naming both places when neither is available.
fn resolve_account_id(username: Option<&str>, default: &str) -> Result<String, PluginError> {
    username
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| Some(default.trim()).filter(|s| !s.is_empty()))
        .map(|s| s.to_string())
        .ok_or_else(|| {
            PluginError::Config(
                "Account ID is required (Username field, or set a default in the plugin settings)"
                    .into(),
            )
        })
}

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

        let account_id = resolve_account_id(p["username"].as_str(), &default_account_id())?;

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
        let account_id = resolve_account_id(p["username"].as_str(), &default_account_id())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_wins_over_default() {
        assert_eq!(resolve_account_id(Some("abc"), "def").unwrap(), "abc");
    }

    #[test]
    fn blank_username_falls_back_to_default() {
        assert_eq!(resolve_account_id(Some("  "), "def").unwrap(), "def");
        assert_eq!(resolve_account_id(None, " def ").unwrap(), "def");
    }

    #[test]
    fn missing_everywhere_is_an_error() {
        let e = resolve_account_id(Some(""), "").unwrap_err().to_string();
        assert!(e.contains("plugin settings"));
    }

    #[test]
    fn apply_settings_sets_default() {
        apply_settings(&serde_json::json!({ "default_account_id": "  acc123 " }));
        assert_eq!(default_account_id(), "acc123");
        // Missing key clears it again.
        apply_settings(&serde_json::json!({}));
        assert_eq!(default_account_id(), "");
    }
}
