// File: main.rs
// Description: Plugin entry point: reads JSON-RPC lines from stdin, dispatches to handlers, writes replies to stdout.
// Author: Jose-Jorge HERNANDEZ
// Company: Parlee Conseiller, Inc.
// Date: 2026-06-15
// Last edit date: 2026-10-09
// Version: 1.1.0

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod client;
mod coords;
mod error;
mod handlers;
mod rpc;
mod utils;

use std::io::{self, BufRead, Write};

use serde_json::Value;

use handlers::{crud, ddl, explain, metadata, query, triggers};
use rpc::{internal_err, ok};

fn main() {
    env_logger::init();

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                log::error!("stdin read error: {}", e);
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Notifications (no `id`) get no reply, per the JSON-RPC spec.
        let response = match handle_line(trimmed) {
            Some(r) => r,
            None => continue,
        };

        let mut serialised = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"serialisation error"},"id":null}"#
                .to_string()
        });
        serialised.push('\n');

        if let Err(e) = out.write_all(serialised.as_bytes()) {
            log::error!("stdout write error: {}", e);
            break;
        }
        let _ = out.flush();
    }
}

/// Handle one request line. Returns `None` for notifications (no `id`), which
/// must not be answered (the host sends `cancel` this way after a timeout).
fn handle_line(line: &str) -> Option<Value> {
    let req: rpc::RpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return Some(rpc::err(Value::Null, -32700, format!("Parse error: {}", e)));
        }
    };

    // No id => notification. Requests are processed strictly one at a time, so
    // a `cancel` can only arrive after its target finished; ignoring it is the
    // spec-compliant behaviour for an unknown/finished id.
    let id = req.id.clone()?;
    let params = &req.params;

    Some(match dispatch(&req.method, params) {
        Ok(v) => ok(id, v),
        Err(error::PluginError::MethodNotFound(m)) => {
            rpc::err(id, rpc::METHOD_NOT_FOUND, format!("Method not found: {}", m))
        }
        Err(e) => internal_err(id, e.to_string()),
    })
}

fn dispatch(method: &str, params: &Value) -> Result<Value, error::PluginError> {
    match method {
        // Lifecycle
        "initialize" => Ok(Value::Null),

        // Connectivity
        "test_connection" => query::test_connection(params),
        "ping" => query::ping(params),

        // Schema discovery
        "get_databases" => query::get_databases(params),
        "get_schemas" => query::get_schemas(params),
        "get_tables" => metadata::get_tables(params),
        "get_columns" => metadata::get_columns(params),
        "get_indexes" => metadata::get_indexes(params),
        "get_foreign_keys" => metadata::get_foreign_keys(params),
        "get_views" => metadata::get_views(params),
        "get_view_definition" => metadata::get_view_definition(params),
        "get_view_columns" => metadata::get_view_columns(params),
        "get_routines" => metadata::get_routines(params),
        "get_routine_parameters" => metadata::get_routine_parameters(params),
        "get_routine_definition" => metadata::get_routine_definition(params),

        // Batch schema methods (ER diagram)
        "get_schema_snapshot" => metadata::get_schema_snapshot(params),
        "get_all_columns_batch" => metadata::get_all_columns_batch(params),
        "get_all_foreign_keys_batch" => metadata::get_all_foreign_keys_batch(params),

        // Triggers
        "get_triggers" => triggers::get_triggers(params),
        "get_trigger_definition" => triggers::get_trigger_definition(params),
        "create_trigger" => triggers::create_trigger(params),
        "drop_trigger" => triggers::drop_trigger(params),

        // Query execution / plans
        "explain_query" => explain::explain_query(params),
        "execute_query" => query::execute_query(params),

        // CRUD
        "insert_record" => crud::insert_record(params),
        "update_record" => crud::update_record(params),
        "delete_record" => crud::delete_record(params),

        // View DDL
        "create_view" => ddl::create_view(params),
        "alter_view" => ddl::alter_view(params),
        "drop_view" => ddl::drop_view(params),

        // Index / FK DDL execution
        "drop_index" => ddl::drop_index(params),
        "drop_foreign_key" => ddl::drop_foreign_key(params),

        // SQL generation (no ConnectionParams)
        "get_create_table_sql" => ddl::get_create_table_sql(params),
        "get_add_column_sql" => ddl::get_add_column_sql(params),
        "get_alter_column_sql" => ddl::get_alter_column_sql(params),
        "get_create_index_sql" => ddl::get_create_index_sql(params),
        "get_create_foreign_key_sql" => ddl::get_create_foreign_key_sql(params),

        unknown => Err(error::PluginError::MethodNotFound(unknown.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_method_returns_32601() {
        let r = handle_line(r#"{"jsonrpc":"2.0","method":"nope","params":{},"id":1}"#).unwrap();
        assert_eq!(r["error"]["code"], -32601);
        assert!(r["error"]["message"].as_str().unwrap().contains("Method not found"));
        assert_eq!(r["id"], 1);
    }

    #[test]
    fn cancel_notification_gets_no_reply() {
        assert!(handle_line(r#"{"jsonrpc":"2.0","method":"cancel","params":{"id":1}}"#).is_none());
    }

    #[test]
    fn malformed_json_returns_parse_error() {
        let r = handle_line("{not json").unwrap();
        assert_eq!(r["error"]["code"], -32700);
    }
}
