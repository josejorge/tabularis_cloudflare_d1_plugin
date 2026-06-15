use serde_json::{json, Value};

use crate::client::{cache_databases, D1Client};
use crate::coords::ConnectionCoords;
use crate::error::PluginError;
use crate::utils::pagination::{has_limit, is_select, paginated_pair};

// ---------------------------------------------------------------------------
// test_connection
// ---------------------------------------------------------------------------

pub fn test_connection(params: &Value) -> Result<Value, PluginError> {
    let (account_id, api_token) = ConnectionCoords::credentials_only(params)?;
    let client = D1Client::new(&api_token, &account_id)?;
    // Listing databases proves the token and account ID are valid.
    client.list_databases()?;
    Ok(json!({ "success": true }))
}

// ---------------------------------------------------------------------------
// ping — lightweight check reusing test_connection
// ---------------------------------------------------------------------------

pub fn ping(params: &Value) -> Result<Value, PluginError> {
    test_connection(params)?;
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------
// get_databases
// ---------------------------------------------------------------------------

pub fn get_databases(params: &Value) -> Result<Value, PluginError> {
    let (account_id, api_token) = ConnectionCoords::credentials_only(params)?;
    let client = D1Client::new(&api_token, &account_id)?;
    let dbs = client.list_databases()?;
    cache_databases(&dbs);
    let names: Vec<&str> = dbs.iter().map(|d| d.name.as_str()).collect();
    Ok(json!(names))
}

// ---------------------------------------------------------------------------
// get_schemas — D1/SQLite has no named schemas
// ---------------------------------------------------------------------------

pub fn get_schemas(_params: &Value) -> Result<Value, PluginError> {
    Ok(json!([]))
}

// ---------------------------------------------------------------------------
// execute_query
// ---------------------------------------------------------------------------

pub fn execute_query(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    if coords.database.is_empty() {
        return Err(PluginError::Config("No database selected".into()));
    }

    let query = params["query"]
        .as_str()
        .ok_or_else(|| PluginError::Config("query is required".into()))?;
    let page = params["page"].as_u64().unwrap_or(1).max(1) as u32;
    let limit = params["limit"].as_u64().map(|v| v as u32);

    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    let start = std::time::Instant::now();

    if is_select(query) && limit.is_some() && !has_limit(query) {
        let page_size = limit.unwrap();
        let (data_sql, count_sql) = paginated_pair(query, page, page_size);

        let results = client.batch(&db_id, &[(&count_sql, vec![]), (&data_sql, vec![])])?;
        let count_set = results.get(0).ok_or_else(|| {
            PluginError::Api("batch returned no count result".into())
        })?;
        let data_set = results.get(1).ok_or_else(|| {
            PluginError::Api("batch returned no data result".into())
        })?;

        let total_rows = count_set
            .results
            .first()
            .and_then(|row| row.get("_count"))
            .and_then(|v| v.as_u64());

        let (columns, rows) = result_set_to_columns_rows(data_set);
        let row_count = rows.len() as u64;
        let offset = (page.saturating_sub(1) as u64) * (page_size as u64);
        let has_more = total_rows.map(|t| offset + row_count < t).unwrap_or(false);

        return Ok(json!({
            "columns": columns,
            "rows": Value::Array(rows),
            "affected_rows": row_count,
            "truncated": false,
            "pagination": {
                "page": page,
                "page_size": page_size,
                "total_rows": total_rows,
                "has_more": has_more
            }
        }));
    }

    // Non-paginated or non-SELECT query.
    let result_set = client.query(&db_id, query, vec![])?;
    let _ = start; // elapsed kept for future telemetry

    if is_select(query) {
        let (columns, rows) = result_set_to_columns_rows(&result_set);
        let row_count = rows.len() as u64;
        Ok(json!({
            "columns": columns,
            "rows": Value::Array(rows),
            "affected_rows": row_count,
            "truncated": false,
            "pagination": null
        }))
    } else {
        let affected = result_set.meta.changes.unsigned_abs();
        Ok(json!({
            "columns": [],
            "rows": [],
            "affected_rows": affected,
            "truncated": false,
            "pagination": null
        }))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert a D1ResultSet into Tabularis (columns, rows) format.
/// Column order is preserved because serde_json::Map is insertion-ordered.
pub fn result_set_to_columns_rows(
    set: &crate::client::D1ResultSet,
) -> (Vec<String>, Vec<Value>) {
    if set.results.is_empty() {
        return (vec![], vec![]);
    }
    let columns: Vec<String> = set.results[0].keys().cloned().collect();
    let rows: Vec<Value> = set
        .results
        .iter()
        .map(|row| {
            let cells: Vec<Value> = columns
                .iter()
                .map(|col| row.get(col).cloned().unwrap_or(Value::Null))
                .collect();
            Value::Array(cells)
        })
        .collect();
    (columns, rows)
}
