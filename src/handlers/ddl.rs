use serde_json::{json, Value};

use crate::client::D1Client;
use crate::coords::ConnectionCoords;
use crate::error::PluginError;
use crate::utils::identifiers::quote_ident;

// ---------------------------------------------------------------------------
// View DDL
// ---------------------------------------------------------------------------

pub fn create_view(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let view_name = params["view_name"]
        .as_str()
        .ok_or_else(|| PluginError::Config("view_name is required".into()))?;
    let definition = params["definition"]
        .as_str()
        .ok_or_else(|| PluginError::Config("definition is required".into()))?;

    let sql = format!(
        "CREATE VIEW {} AS {}",
        quote_ident(view_name),
        definition.trim_end_matches(';')
    );
    execute_ddl(&coords, &sql)?;
    Ok(Value::Null)
}

pub fn alter_view(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let view_name = params["view_name"]
        .as_str()
        .ok_or_else(|| PluginError::Config("view_name is required".into()))?;
    let definition = params["definition"]
        .as_str()
        .ok_or_else(|| PluginError::Config("definition is required".into()))?;

    // SQLite has no ALTER VIEW — drop and recreate.
    let drop_sql = format!("DROP VIEW IF EXISTS {}", quote_ident(view_name));
    let create_sql = format!(
        "CREATE VIEW {} AS {}",
        quote_ident(view_name),
        definition.trim_end_matches(';')
    );
    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;
    client.batch(&db_id, &[(&drop_sql, vec![]), (&create_sql, vec![])])?;
    Ok(Value::Null)
}

pub fn drop_view(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let view_name = params["view_name"]
        .as_str()
        .ok_or_else(|| PluginError::Config("view_name is required".into()))?;

    let sql = format!("DROP VIEW IF EXISTS {}", quote_ident(view_name));
    execute_ddl(&coords, &sql)?;
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------
// Index DDL
// ---------------------------------------------------------------------------

pub fn drop_index(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let index_name = params["index_name"]
        .as_str()
        .ok_or_else(|| PluginError::Config("index_name is required".into()))?;

    let sql = format!("DROP INDEX IF EXISTS {}", quote_ident(index_name));
    execute_ddl(&coords, &sql)?;
    Ok(Value::Null)
}

// D1/SQLite does not support dropping FK constraints without recreating the table.
pub fn drop_foreign_key(_params: &Value) -> Result<Value, PluginError> {
    Err(PluginError::Api(
        "Dropping individual foreign key constraints is not supported in SQLite/D1. \
         Recreate the table without the constraint instead."
            .into(),
    ))
}

// ---------------------------------------------------------------------------
// SQL generation — these methods return Vec<String> (DDL statements)
// and do NOT receive ConnectionParams (see driver.rs).
// ---------------------------------------------------------------------------

pub fn get_create_table_sql(params: &Value) -> Result<Value, PluginError> {
    let table_name = params["table_name"]
        .as_str()
        .ok_or_else(|| PluginError::Config("table_name is required".into()))?;
    let columns = params["columns"]
        .as_array()
        .ok_or_else(|| PluginError::Config("columns is required".into()))?;

    // First pass: count PK columns so we know whether to use an inline or
    // table-level PRIMARY KEY constraint.
    let pk_count = columns
        .iter()
        .filter(|c| c["is_pk"].as_bool().unwrap_or(false))
        .count();

    let mut col_defs: Vec<String> = Vec::new();
    let mut pk_cols: Vec<String> = Vec::new();

    for col in columns {
        let name = col["name"].as_str().unwrap_or("col");
        let data_type = col["data_type"].as_str().unwrap_or("TEXT").to_uppercase();
        let is_pk = col["is_pk"].as_bool().unwrap_or(false);
        let is_nullable = col["is_nullable"].as_bool().unwrap_or(true);
        let is_auto_inc = col["is_auto_increment"].as_bool().unwrap_or(false);
        let default_value = col["default_value"].as_str();

        if is_pk {
            pk_cols.push(quote_ident(name));
        }

        let mut def = format!("{} {}", quote_ident(name), data_type);

        // Use inline PRIMARY KEY only for the sole PK column when it is INTEGER
        // (SQLite rowid alias). Every other case uses a table constraint below.
        if is_pk && pk_count == 1 && data_type == "INTEGER" {
            def += " PRIMARY KEY";
            if is_auto_inc {
                def += " AUTOINCREMENT";
            }
        } else {
            if !is_nullable || is_pk {
                def += " NOT NULL";
            }
            if let Some(dv) = default_value {
                def += &format!(" DEFAULT {}", dv);
            }
        }
        col_defs.push(def);
    }

    // Table-level PRIMARY KEY for composite keys or non-INTEGER single keys.
    let needs_table_pk = pk_count > 1
        || (pk_count == 1
            && columns.iter().any(|c| {
                c["is_pk"].as_bool().unwrap_or(false)
                    && c["data_type"].as_str().unwrap_or("").to_uppercase() != "INTEGER"
            }));

    if needs_table_pk && !pk_cols.is_empty() {
        col_defs.push(format!("PRIMARY KEY ({})", pk_cols.join(", ")));
    }

    let sql = format!(
        "CREATE TABLE {} (\n  {}\n)",
        quote_ident(table_name),
        col_defs.join(",\n  ")
    );

    Ok(json!([sql]))
}

pub fn get_add_column_sql(params: &Value) -> Result<Value, PluginError> {
    let table = params["table"]
        .as_str()
        .ok_or_else(|| PluginError::Config("table is required".into()))?;
    let col = &params["column"];
    let name = col["name"].as_str().unwrap_or("col");
    let data_type = col["data_type"].as_str().unwrap_or("TEXT").to_uppercase();
    let is_nullable = col["is_nullable"].as_bool().unwrap_or(true);
    let default_value = col["default_value"].as_str();

    let mut def = format!("{} {}", quote_ident(name), data_type);
    if !is_nullable {
        // SQLite requires a DEFAULT when adding a NOT NULL column to an existing table.
        let dv = default_value.unwrap_or("''");
        def += &format!(" NOT NULL DEFAULT {}", dv);
    } else if let Some(dv) = default_value {
        def += &format!(" DEFAULT {}", dv);
    }

    let sql = format!("ALTER TABLE {} ADD COLUMN {}", quote_ident(table), def);
    Ok(json!([sql]))
}

// SQLite does not support ALTER COLUMN.
pub fn get_alter_column_sql(_params: &Value) -> Result<Value, PluginError> {
    Err(PluginError::Api(
        "ALTER COLUMN is not supported in SQLite/D1. \
         Recreate the table with the desired schema instead."
            .into(),
    ))
}

pub fn get_create_index_sql(params: &Value) -> Result<Value, PluginError> {
    let table = params["table"]
        .as_str()
        .ok_or_else(|| PluginError::Config("table is required".into()))?;
    let index_name = params["index_name"]
        .as_str()
        .ok_or_else(|| PluginError::Config("index_name is required".into()))?;
    let columns = params["columns"]
        .as_array()
        .ok_or_else(|| PluginError::Config("columns is required".into()))?;
    let is_unique = params["is_unique"].as_bool().unwrap_or(false);

    let col_list: Vec<String> = columns
        .iter()
        .filter_map(|v| v.as_str())
        .map(quote_ident)
        .collect();

    let unique_kw = if is_unique { "UNIQUE " } else { "" };
    let sql = format!(
        "CREATE {}INDEX {} ON {} ({})",
        unique_kw,
        quote_ident(index_name),
        quote_ident(table),
        col_list.join(", ")
    );

    Ok(json!([sql]))
}

// In SQLite, foreign keys must be defined in CREATE TABLE — runtime addition
// via ALTER TABLE is not supported.
pub fn get_create_foreign_key_sql(_params: &Value) -> Result<Value, PluginError> {
    Err(PluginError::Api(
        "Adding foreign key constraints to existing tables is not supported in SQLite/D1. \
         Include the FOREIGN KEY clause in CREATE TABLE instead."
            .into(),
    ))
}

// ---------------------------------------------------------------------------
// Internal helper
// ---------------------------------------------------------------------------

fn execute_ddl(coords: &ConnectionCoords, sql: &str) -> Result<(), PluginError> {
    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;
    client.query(&db_id, sql, vec![])?;
    Ok(())
}
