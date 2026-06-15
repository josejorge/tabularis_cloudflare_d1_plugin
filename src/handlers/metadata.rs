use serde_json::{json, Value};

use crate::client::D1Client;
use crate::coords::ConnectionCoords;
use crate::error::PluginError;
use crate::utils::identifiers::quote_ident;
use crate::utils::types::normalize;

// ---------------------------------------------------------------------------
// get_tables
// ---------------------------------------------------------------------------

pub fn get_tables(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    let sql = "SELECT name FROM sqlite_master \
               WHERE type = 'table' \
               AND name NOT LIKE 'sqlite_%' \
               AND name NOT LIKE '_cf_%' \
               ORDER BY name";

    let rs = client.query(&db_id, sql, vec![])?;
    let tables: Vec<Value> = rs
        .results
        .iter()
        .filter_map(|row| row.get("name").and_then(|v| v.as_str()))
        .map(|name| json!({ "name": name }))
        .collect();

    Ok(json!(tables))
}

// ---------------------------------------------------------------------------
// get_columns
// Tabularis TableColumn: { name, data_type, is_pk, is_nullable, is_auto_increment,
//                          default_value?, character_maximum_length? }
// ---------------------------------------------------------------------------

pub fn get_columns(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let table = params["table"]
        .as_str()
        .ok_or_else(|| PluginError::Config("table is required".into()))?;

    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    let sql = format!("PRAGMA table_info({})", quote_ident(table));
    let rs = client.query(&db_id, &sql, vec![])?;

    // PRAGMA table_info columns: cid, name, type, notnull, dflt_value, pk
    let columns: Vec<Value> = rs
        .results
        .iter()
        .map(|row| {
            let name = row.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let raw_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let data_type = normalize(raw_type);
            let not_null = row.get("notnull").and_then(|v| v.as_i64()).unwrap_or(0) != 0;
            let pk = row.get("pk").and_then(|v| v.as_i64()).unwrap_or(0) != 0;
            let dflt = row.get("dflt_value").and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    Some(v.as_str().unwrap_or(&v.to_string()).to_string())
                }
            });
            // INTEGER PRIMARY KEY is implicitly auto-increment in SQLite
            let is_auto_increment = pk && data_type == "INTEGER";

            json!({
                "name": name,
                "data_type": data_type,
                "is_pk": pk,
                "is_nullable": !not_null,
                "is_auto_increment": is_auto_increment,
                "default_value": dflt
            })
        })
        .collect();

    Ok(json!(columns))
}

// ---------------------------------------------------------------------------
// get_indexes
// Tabularis Index: { name, column_name, is_unique, is_primary, seq_in_index }
// Returns one row per index-column pair (same shape as MySQL SHOW INDEX).
// ---------------------------------------------------------------------------

pub fn get_indexes(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let table = params["table"]
        .as_str()
        .ok_or_else(|| PluginError::Config("table is required".into()))?;

    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    // First get the index list for this table.
    let list_sql = format!("PRAGMA index_list({})", quote_ident(table));
    let list_rs = client.query(&db_id, &list_sql, vec![])?;

    let mut rows: Vec<Value> = vec![];

    for idx_row in &list_rs.results {
        let idx_name = match idx_row.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let is_unique = idx_row
            .get("unique")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            != 0;
        let origin = idx_row
            .get("origin")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        // "pk" origin = auto-created PRIMARY KEY index; "c" = explicit CREATE INDEX
        let is_primary = origin == "pk";

        let info_sql = format!("PRAGMA index_info({})", quote_ident(&idx_name));
        let info_rs = client.query(&db_id, &info_sql, vec![])?;

        for col_row in &info_rs.results {
            let col_name = col_row
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let seq = col_row
                .get("seqno")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;

            rows.push(json!({
                "name": idx_name,
                "column_name": col_name,
                "is_unique": is_unique,
                "is_primary": is_primary,
                "seq_in_index": seq + 1
            }));
        }
    }

    Ok(json!(rows))
}

// ---------------------------------------------------------------------------
// get_foreign_keys
// Tabularis ForeignKey: { name, column_name, ref_table, ref_column, on_delete?, on_update? }
// ---------------------------------------------------------------------------

pub fn get_foreign_keys(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let table = params["table"]
        .as_str()
        .ok_or_else(|| PluginError::Config("table is required".into()))?;

    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    let sql = format!("PRAGMA foreign_key_list({})", quote_ident(table));
    let rs = client.query(&db_id, &sql, vec![])?;

    // PRAGMA foreign_key_list columns: id, seq, table, from, to, on_update, on_delete, match
    let fks: Vec<Value> = rs
        .results
        .iter()
        .map(|row| {
            let fk_id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let seq = row.get("seq").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = format!("fk_{}_{}", table, fk_id * 100 + seq);
            let col = row.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let ref_table = row.get("table").and_then(|v| v.as_str()).unwrap_or("");
            let ref_col = row.get("to").and_then(|v| v.as_str()).unwrap_or("");
            let on_delete = normalize_action(
                row.get("on_delete").and_then(|v| v.as_str()).unwrap_or(""),
            );
            let on_update = normalize_action(
                row.get("on_update").and_then(|v| v.as_str()).unwrap_or(""),
            );

            json!({
                "name": name,
                "column_name": col,
                "ref_table": ref_table,
                "ref_column": ref_col,
                "on_delete": on_delete,
                "on_update": on_update
            })
        })
        .collect();

    Ok(json!(fks))
}

fn normalize_action(action: &str) -> Option<String> {
    match action.to_uppercase().as_str() {
        "NO ACTION" | "" => None,
        other => Some(other.to_string()),
    }
}

// ---------------------------------------------------------------------------
// get_views
// Tabularis ViewInfo: { name, definition? }
// ---------------------------------------------------------------------------

pub fn get_views(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    let sql = "SELECT name, sql FROM sqlite_master \
               WHERE type = 'view' \
               ORDER BY name";
    let rs = client.query(&db_id, sql, vec![])?;

    let views: Vec<Value> = rs
        .results
        .iter()
        .map(|row| {
            let name = row.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let def = row
                .get("sql")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            json!({ "name": name, "definition": def })
        })
        .collect();

    Ok(json!(views))
}

// ---------------------------------------------------------------------------
// get_view_definition
// ---------------------------------------------------------------------------

pub fn get_view_definition(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let view_name = params["view_name"]
        .as_str()
        .ok_or_else(|| PluginError::Config("view_name is required".into()))?;

    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    let sql = format!(
        "SELECT sql FROM sqlite_master WHERE type = 'view' AND name = ?1"
    );
    let rs = client.query(&db_id, &sql, vec![Value::String(view_name.to_string())])?;

    let definition = rs
        .results
        .first()
        .and_then(|row| row.get("sql"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(Value::String(definition))
}

// ---------------------------------------------------------------------------
// get_view_columns — reuse PRAGMA table_info which works for views in SQLite
// ---------------------------------------------------------------------------

pub fn get_view_columns(params: &Value) -> Result<Value, PluginError> {
    // Build a synthetic params object that points table to the view name.
    let mut synth = params.clone();
    if let Some(obj) = synth.as_object_mut() {
        if let Some(view_name) = params["view_name"].as_str() {
            obj.insert("table".into(), Value::String(view_name.to_string()));
        }
    }
    get_columns(&synth)
}

// ---------------------------------------------------------------------------
// get_schema_snapshot
// Tabularis TableSchema: { name, columns: Vec<TableColumn>, foreign_keys: Vec<ForeignKey> }
// ---------------------------------------------------------------------------

pub fn get_schema_snapshot(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    // Fetch all table names first.
    let tables_sql = "SELECT name FROM sqlite_master \
                      WHERE type = 'table' \
                      AND name NOT LIKE 'sqlite_%' \
                      AND name NOT LIKE '_cf_%' \
                      ORDER BY name";
    let tables_rs = client.query(&db_id, tables_sql, vec![])?;
    let table_names: Vec<String> = tables_rs
        .results
        .iter()
        .filter_map(|r| r.get("name").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .collect();

    // For each table, fetch columns and FKs using the batch endpoint (2
    // requests per table). We chunk to avoid overly large batch requests.
    let mut snapshots: Vec<Value> = Vec::with_capacity(table_names.len());

    for table in &table_names {
        let col_sql = format!("PRAGMA table_info({})", quote_ident(table));
        let fk_sql = format!("PRAGMA foreign_key_list({})", quote_ident(table));

        let results = client.batch(&db_id, &[(&col_sql, vec![]), (&fk_sql, vec![])])?;
        let col_rs = results.get(0).ok_or_else(|| PluginError::Api("missing col batch result".into()))?;
        let fk_rs = results.get(1).ok_or_else(|| PluginError::Api("missing fk batch result".into()))?;

        // Build columns.
        let columns: Vec<Value> = col_rs
            .results
            .iter()
            .map(|row| {
                let name = row.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let raw_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let data_type = normalize(raw_type);
                let not_null = row.get("notnull").and_then(|v| v.as_i64()).unwrap_or(0) != 0;
                let pk = row.get("pk").and_then(|v| v.as_i64()).unwrap_or(0) != 0;
                let dflt = row.get("dflt_value").and_then(|v| {
                    if v.is_null() { None } else { Some(v.as_str().unwrap_or(&v.to_string()).to_string()) }
                });
                json!({
                    "name": name,
                    "data_type": data_type,
                    "is_pk": pk,
                    "is_nullable": !not_null,
                    "is_auto_increment": pk && data_type == "INTEGER",
                    "default_value": dflt
                })
            })
            .collect();

        // Build foreign keys.
        let fks: Vec<Value> = fk_rs
            .results
            .iter()
            .map(|row| {
                let fk_id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let seq = row.get("seq").and_then(|v| v.as_i64()).unwrap_or(0);
                let fk_name = format!("fk_{}_{}", table, fk_id * 100 + seq);
                json!({
                    "name": fk_name,
                    "column_name": row.get("from").and_then(|v| v.as_str()).unwrap_or(""),
                    "ref_table": row.get("table").and_then(|v| v.as_str()).unwrap_or(""),
                    "ref_column": row.get("to").and_then(|v| v.as_str()).unwrap_or(""),
                    "on_delete": normalize_action(row.get("on_delete").and_then(|v| v.as_str()).unwrap_or("")),
                    "on_update": normalize_action(row.get("on_update").and_then(|v| v.as_str()).unwrap_or(""))
                })
            })
            .collect();

        snapshots.push(json!({
            "name": table,
            "columns": columns,
            "foreign_keys": fks
        }));
    }

    Ok(json!(snapshots))
}

// ---------------------------------------------------------------------------
// get_all_columns_batch  →  { tableName: [TableColumn] }
// ---------------------------------------------------------------------------

pub fn get_all_columns_batch(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    // Fetch all table names then batch-query columns.
    let tables_sql = "SELECT name FROM sqlite_master \
                      WHERE type = 'table' \
                      AND name NOT LIKE 'sqlite_%' \
                      AND name NOT LIKE '_cf_%' \
                      ORDER BY name";
    let tables_rs = client.query(&db_id, tables_sql, vec![])?;
    let table_names: Vec<String> = tables_rs
        .results
        .iter()
        .filter_map(|r| r.get("name").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .collect();

    let mut out = serde_json::Map::new();

    for table in &table_names {
        let sql = format!("PRAGMA table_info({})", quote_ident(table));
        let rs = client.query(&db_id, &sql, vec![])?;
        let cols: Vec<Value> = rs
            .results
            .iter()
            .map(|row| {
                let raw_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let data_type = normalize(raw_type);
                let pk = row.get("pk").and_then(|v| v.as_i64()).unwrap_or(0) != 0;
                let not_null = row.get("notnull").and_then(|v| v.as_i64()).unwrap_or(0) != 0;
                let dflt = row.get("dflt_value").and_then(|v| {
                    if v.is_null() { None } else { Some(v.as_str().unwrap_or(&v.to_string()).to_string()) }
                });
                json!({
                    "name": row.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "data_type": data_type,
                    "is_pk": pk,
                    "is_nullable": !not_null,
                    "is_auto_increment": pk && data_type == "INTEGER",
                    "default_value": dflt
                })
            })
            .collect();
        out.insert(table.clone(), Value::Array(cols));
    }

    Ok(Value::Object(out))
}

// ---------------------------------------------------------------------------
// get_all_foreign_keys_batch  →  { tableName: [ForeignKey] }
// ---------------------------------------------------------------------------

pub fn get_all_foreign_keys_batch(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    let tables_sql = "SELECT name FROM sqlite_master \
                      WHERE type = 'table' \
                      AND name NOT LIKE 'sqlite_%' \
                      AND name NOT LIKE '_cf_%' \
                      ORDER BY name";
    let tables_rs = client.query(&db_id, tables_sql, vec![])?;
    let table_names: Vec<String> = tables_rs
        .results
        .iter()
        .filter_map(|r| r.get("name").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .collect();

    let mut out = serde_json::Map::new();

    for table in &table_names {
        let sql = format!("PRAGMA foreign_key_list({})", quote_ident(table));
        let rs = client.query(&db_id, &sql, vec![])?;
        let fks: Vec<Value> = rs
            .results
            .iter()
            .map(|row| {
                let fk_id = row.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let seq = row.get("seq").and_then(|v| v.as_i64()).unwrap_or(0);
                json!({
                    "name": format!("fk_{}_{}", table, fk_id * 100 + seq),
                    "column_name": row.get("from").and_then(|v| v.as_str()).unwrap_or(""),
                    "ref_table": row.get("table").and_then(|v| v.as_str()).unwrap_or(""),
                    "ref_column": row.get("to").and_then(|v| v.as_str()).unwrap_or(""),
                    "on_delete": normalize_action(row.get("on_delete").and_then(|v| v.as_str()).unwrap_or("")),
                    "on_update": normalize_action(row.get("on_update").and_then(|v| v.as_str()).unwrap_or(""))
                })
            })
            .collect();
        out.insert(table.clone(), Value::Array(fks));
    }

    Ok(Value::Object(out))
}

// ---------------------------------------------------------------------------
// Routines — D1/SQLite does not support stored procedures or functions
// ---------------------------------------------------------------------------

pub fn get_routines(_params: &Value) -> Result<Value, PluginError> {
    Ok(json!([]))
}

pub fn get_routine_parameters(_params: &Value) -> Result<Value, PluginError> {
    Ok(json!([]))
}

pub fn get_routine_definition(_params: &Value) -> Result<Value, PluginError> {
    Ok(Value::String(String::new()))
}

