#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod client;
mod coords;
mod error;
mod handlers;
mod rpc;
mod utils;

use std::io::{self, BufRead, Write};

use serde_json::Value;

use handlers::{crud, ddl, metadata, query};
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

        let response = handle_line(trimmed);

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

fn handle_line(line: &str) -> Value {
    let req: rpc::RpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return rpc::err(Value::Null, -32700, format!("Parse error: {}", e));
        }
    };

    let id = req.id.clone();
    let params = &req.params;

    match dispatch(&req.method, params) {
        Ok(v) => ok(id, v),
        Err(e) => internal_err(id, e.to_string()),
    }
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

        // Query execution
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

        unknown => Err(error::PluginError::Api(format!(
            "Method '{}' not implemented",
            unknown
        ))),
    }
}
