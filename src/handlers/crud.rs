use serde_json::{json, Value};

use crate::client::D1Client;
use crate::coords::ConnectionCoords;
use crate::error::PluginError;
use crate::utils::identifiers::quote_ident;
use crate::utils::values::to_d1_param;

// ---------------------------------------------------------------------------
// insert_record
// ---------------------------------------------------------------------------

pub fn insert_record(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let table = params["table"]
        .as_str()
        .ok_or_else(|| PluginError::Config("table is required".into()))?;
    let data = params["data"]
        .as_object()
        .ok_or_else(|| PluginError::Config("data is required".into()))?;

    if data.is_empty() {
        return Err(PluginError::Config("data must contain at least one column".into()));
    }

    let cols: Vec<&str> = data.keys().map(|k| k.as_str()).collect();
    let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("?{}", i)).collect();

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote_ident(table),
        cols.iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", "),
        placeholders.join(", ")
    );

    let bind_params: Vec<Value> = cols
        .iter()
        .map(|c| to_d1_param(data.get(*c).unwrap_or(&Value::Null)))
        .collect();

    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;
    let rs = client.query(&db_id, &sql, bind_params)?;

    Ok(json!(rs.meta.changes.unsigned_abs()))
}

// ---------------------------------------------------------------------------
// update_record
// ---------------------------------------------------------------------------

pub fn update_record(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let table = params["table"]
        .as_str()
        .ok_or_else(|| PluginError::Config("table is required".into()))?;
    let pk_col = params["pk_col"]
        .as_str()
        .ok_or_else(|| PluginError::Config("pk_col is required".into()))?;
    let pk_val = &params["pk_val"];
    let col_name = params["col_name"]
        .as_str()
        .ok_or_else(|| PluginError::Config("col_name is required".into()))?;
    let new_val = &params["new_val"];

    let sql = format!(
        "UPDATE {} SET {} = ?1 WHERE {} = ?2",
        quote_ident(table),
        quote_ident(col_name),
        quote_ident(pk_col)
    );
    let bind_params = vec![to_d1_param(new_val), to_d1_param(pk_val)];

    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;
    let rs = client.query(&db_id, &sql, bind_params)?;

    Ok(json!(rs.meta.changes.unsigned_abs()))
}

// ---------------------------------------------------------------------------
// delete_record
// ---------------------------------------------------------------------------

pub fn delete_record(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    let table = params["table"]
        .as_str()
        .ok_or_else(|| PluginError::Config("table is required".into()))?;
    let pk_col = params["pk_col"]
        .as_str()
        .ok_or_else(|| PluginError::Config("pk_col is required".into()))?;
    let pk_val = &params["pk_val"];

    let sql = format!(
        "DELETE FROM {} WHERE {} = ?1",
        quote_ident(table),
        quote_ident(pk_col)
    );

    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;
    let rs = client.query(&db_id, &sql, vec![to_d1_param(pk_val)])?;

    Ok(json!(rs.meta.changes.unsigned_abs()))
}
