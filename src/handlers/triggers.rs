// File: triggers.rs
// Description: Trigger handlers (get_triggers, get_trigger_definition, create_trigger, drop_trigger) backed by sqlite_master.
// Author: Jose-Jorge HERNANDEZ
// Company: Parlee Conseiller, Inc.
// Date: 2026-10-09
// Last edit date: 2026-10-09
// Version: 1.0.0

use serde_json::{json, Value};

use crate::client::D1Client;
use crate::coords::ConnectionCoords;
use crate::error::PluginError;
use crate::utils::identifiers::quote_ident;

// ---------------------------------------------------------------------------
// get_triggers
// Tabularis TriggerInfo: { name, table_name, event, timing, definition? }
// ---------------------------------------------------------------------------

pub fn get_triggers(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    let sql = "SELECT name, tbl_name, sql FROM sqlite_master \
               WHERE type = 'trigger' \
               ORDER BY name";
    let rs = client.query(&db_id, sql, vec![])?;

    let triggers: Vec<Value> = rs
        .results
        .iter()
        .map(|row| {
            let name = row.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let table = row.get("tbl_name").and_then(|v| v.as_str()).unwrap_or("");
            let def = row.get("sql").and_then(|v| v.as_str());
            let (timing, event) = parse_trigger_header(def.unwrap_or(""));
            json!({
                "name": name,
                "table_name": table,
                "event": event,
                "timing": timing,
                "definition": def
            })
        })
        .collect();

    Ok(json!(triggers))
}

// ---------------------------------------------------------------------------
// get_trigger_definition
// ---------------------------------------------------------------------------

pub fn get_trigger_definition(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let name = params["trigger_name"]
        .as_str()
        .ok_or_else(|| PluginError::Config("trigger_name is required".into()))?;

    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    let rs = client.query(
        &db_id,
        "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
        vec![json!(name)],
    )?;
    let def = rs
        .results
        .first()
        .and_then(|row| row.get("sql"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| PluginError::Api(format!("Trigger '{}' not found", name)))?;
    Ok(json!(def))
}

// ---------------------------------------------------------------------------
// create_trigger / drop_trigger
// ---------------------------------------------------------------------------

pub fn create_trigger(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let trigger_sql = params["trigger_sql"]
        .as_str()
        .ok_or_else(|| PluginError::Config("trigger_sql is required".into()))?;
    run(&coords, trigger_sql)?;
    Ok(Value::Null)
}

pub fn drop_trigger(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let name = params["trigger_name"]
        .as_str()
        .ok_or_else(|| PluginError::Config("trigger_name is required".into()))?;
    run(
        &coords,
        &format!("DROP TRIGGER IF EXISTS {}", quote_ident(name)),
    )?;
    Ok(Value::Null)
}

/// Executes one statement against the connection's database.
fn run(coords: &ConnectionCoords, sql: &str) -> Result<(), PluginError> {
    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;
    client.query(&db_id, sql, vec![])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Header parsing
// ---------------------------------------------------------------------------

/// Extracts (timing, event) from a `CREATE TRIGGER` statement.
///
/// SQLite stores only the original SQL, so the header is parsed from text:
/// `CREATE TRIGGER name [BEFORE|AFTER|INSTEAD OF] {INSERT|DELETE|UPDATE [OF ..]} ON tbl ...`.
/// A missing timing keyword means BEFORE (SQLite's default). Unparseable SQL
/// yields ("BEFORE", "") so the UI still lists the trigger.
fn parse_trigger_header(sql: &str) -> (String, String) {
    // Normalise whitespace/case so newlines and tabs don't hide keywords.
    let norm = sql
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase();
    // The header ends where the table clause starts.
    let head = match norm.find(" ON ") {
        Some(pos) => &norm[..pos],
        None => norm.as_str(),
    };

    let (timing, after_timing) = if let Some(p) = head.find(" INSTEAD OF ") {
        ("INSTEAD OF", p)
    } else if let Some(p) = head.find(" AFTER ") {
        ("AFTER", p)
    } else if let Some(p) = head.find(" BEFORE ") {
        ("BEFORE", p)
    } else {
        ("BEFORE", 0)
    };

    // Search for the event only after the timing keyword so a trigger *named*
    // e.g. "update_log" is not mistaken for the event.
    let tail = &head[after_timing..];
    let event = ["INSERT", "DELETE", "UPDATE"]
        .iter()
        .filter_map(|e| tail.find(&format!(" {}", e)).map(|p| (p, *e)))
        .min_by_key(|(p, _)| *p)
        .map(|(_, e)| e)
        .unwrap_or("");

    (timing.to_string(), event.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(sql: &str) -> (String, String) {
        parse_trigger_header(sql)
    }

    #[test]
    fn after_insert() {
        assert_eq!(
            h("CREATE TRIGGER trg AFTER INSERT ON users BEGIN SELECT 1; END"),
            ("AFTER".into(), "INSERT".into())
        );
    }

    #[test]
    fn multiline_instead_of_update_of() {
        assert_eq!(
            h("CREATE TRIGGER \"v trg\"\n  INSTEAD OF\n UPDATE OF name ON v\nBEGIN SELECT 1; END"),
            ("INSTEAD OF".into(), "UPDATE".into())
        );
    }

    #[test]
    fn default_timing_is_before() {
        assert_eq!(
            h("CREATE TRIGGER t DELETE ON x BEGIN SELECT 1; END"),
            ("BEFORE".into(), "DELETE".into())
        );
    }

    #[test]
    fn name_containing_event_word_is_not_the_event() {
        assert_eq!(
            h("CREATE TRIGGER update_log AFTER DELETE ON t BEGIN SELECT 1; END"),
            ("AFTER".into(), "DELETE".into())
        );
    }

    #[test]
    fn garbage_does_not_panic() {
        assert_eq!(h(""), ("BEFORE".into(), "".into()));
    }
}
