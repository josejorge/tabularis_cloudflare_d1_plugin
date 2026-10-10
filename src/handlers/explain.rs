// File: explain.rs
// Description: explain_query handler: runs EXPLAIN QUERY PLAN on D1 and returns the plan as a parsed tree for Tabularis Visual EXPLAIN.
// Author: Jose-Jorge HERNANDEZ
// Company: Parlee Conseiller, Inc.
// Date: 2026-10-09
// Last edit date: 2026-10-09
// Version: 1.0.0

use serde_json::{json, Value};

use crate::client::D1Client;
use crate::coords::ConnectionCoords;
use crate::error::PluginError;
use crate::utils::pagination::strip_trailing_semicolon;

/// One decoded `EXPLAIN QUERY PLAN` row.
struct EqpRow {
    id: i64,
    parent: i64,
    detail: String,
}

// ---------------------------------------------------------------------------
// explain_query
// ---------------------------------------------------------------------------

/// Runs `EXPLAIN QUERY PLAN` and returns the historical parsed-plan shape
/// (works on every Tabularis release that supports plugin EXPLAIN, unlike the
/// raw payload shape which needs v0.23+). SQLite has no cost model or ANALYZE
/// mode, so `analyze` is ignored and cost/timing fields stay null.
pub fn explain_query(params: &Value) -> Result<Value, PluginError> {
    let coords = ConnectionCoords::from_params(params)?;
    if coords.database.is_empty() {
        return Err(PluginError::Config("No database selected".into()));
    }
    let query = params["query"]
        .as_str()
        .ok_or_else(|| PluginError::Config("query is required".into()))?;

    let client = D1Client::new(&coords.api_token, &coords.account_id)?;
    let db_id = client.resolve_db_id(&coords.database)?;

    let sql = format!("EXPLAIN QUERY PLAN {}", strip_trailing_semicolon(query));
    let rs = client.query(&db_id, &sql, vec![])?;

    let rows: Vec<EqpRow> = rs
        .results
        .iter()
        .map(|r| EqpRow {
            id: r.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
            parent: r.get("parent").and_then(|v| v.as_i64()).unwrap_or(0),
            detail: r
                .get("detail")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    if rows.is_empty() {
        return Err(PluginError::Api(
            "EXPLAIN QUERY PLAN returned no output".into(),
        ));
    }
    Ok(build_plan(&rows, query))
}

/// Builds the parsed-plan JSON. EQP has no single root (top-level rows have
/// parent 0), so a synthetic "Query Plan" node wraps them.
fn build_plan(rows: &[EqpRow], query: &str) -> Value {
    let mut next_id = 0u32;
    let root_id = alloc_id(&mut next_id);
    let children = children_of(rows, 0, &mut next_id, 0);
    let raw = rows
        .iter()
        .map(|r| format!("{}|{}|{}", r.id, r.parent, r.detail))
        .collect::<Vec<_>>()
        .join("\n");

    json!({
        "root": node(root_id, "Query Plan", None, None, "", children),
        "planning_time_ms": null,
        "execution_time_ms": null,
        "original_query": query,
        "driver": "sqlite",
        "has_analyze_data": false,
        "raw_output": raw
    })
}

/// Maximum nesting followed when building the tree; guards against malformed
/// parent links ever recursing forever.
const MAX_DEPTH: usize = 64;

fn children_of(rows: &[EqpRow], parent: i64, next_id: &mut u32, depth: usize) -> Vec<Value> {
    if depth >= MAX_DEPTH {
        return vec![];
    }
    rows.iter()
        // `id != parent` skips a self-referencing row.
        .filter(|r| r.parent == parent && r.id != parent)
        .map(|r| {
            let (node_type, relation, index) = parse_detail(&r.detail);
            let id = alloc_id(next_id);
            let kids = children_of(rows, r.id, next_id, depth + 1);
            node(id, &node_type, relation, index, &r.detail, kids)
        })
        .collect()
}

fn alloc_id(next_id: &mut u32) -> String {
    let id = format!("node-{}", next_id);
    *next_id += 1;
    id
}

/// One plan node in the shape Tabularis' Visual EXPLAIN expects.
fn node(
    id: String,
    node_type: &str,
    relation: Option<String>,
    index_condition: Option<String>,
    detail: &str,
    children: Vec<Value>,
) -> Value {
    json!({
        "id": id,
        "node_type": node_type,
        "relation": relation,
        "startup_cost": null,
        "total_cost": null,
        "plan_rows": null,
        "actual_rows": null,
        "actual_time_ms": null,
        "actual_loops": null,
        "buffers_hit": null,
        "buffers_read": null,
        "filter": null,
        "index_condition": index_condition,
        "join_type": null,
        "hash_condition": null,
        "extra": { "detail": detail },
        "children": children
    })
}

/// Splits an EQP `detail` string into (node type, relation, index condition).
///
/// `SCAN users` / `SCAN TABLE users` / `SEARCH users USING INDEX idx (a=?)`
/// become Scan / Search nodes; anything else (`USE TEMP B-TREE FOR ORDER BY`,
/// `CO-ROUTINE x`, ...) keeps its full text as the node type.
fn parse_detail(detail: &str) -> (String, Option<String>, Option<String>) {
    let upper = detail.to_ascii_uppercase();
    let node_type = if upper.starts_with("SCAN") {
        "Scan"
    } else if upper.starts_with("SEARCH") {
        "Search"
    } else {
        return (detail.to_string(), None, None);
    };

    let mut tokens = detail.split_whitespace().skip(1);
    let mut relation = tokens.next();
    // Older SQLite versions print "SCAN TABLE users".
    if relation.map(|t| t.eq_ignore_ascii_case("TABLE")) == Some(true) {
        relation = tokens.next();
    }
    let index = upper
        .find(" USING ")
        .map(|pos| detail[pos + " USING ".len()..].trim().to_string())
        .filter(|s| !s.is_empty());

    (
        node_type.to_string(),
        relation.map(|s| s.to_string()),
        index,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_and_search_details() {
        assert_eq!(
            parse_detail("SCAN users"),
            ("Scan".into(), Some("users".into()), None)
        );
        assert_eq!(
            parse_detail("SCAN TABLE users"),
            ("Scan".into(), Some("users".into()), None)
        );
        assert_eq!(
            parse_detail("SEARCH users USING INDEX idx_email (email=?)"),
            (
                "Search".into(),
                Some("users".into()),
                Some("INDEX idx_email (email=?)".into())
            )
        );
        assert_eq!(
            parse_detail("USE TEMP B-TREE FOR ORDER BY"),
            ("USE TEMP B-TREE FOR ORDER BY".into(), None, None)
        );
    }

    #[test]
    fn tree_is_built_from_parent_links() {
        let rows = vec![
            EqpRow { id: 3, parent: 0, detail: "SCAN a".into() },
            EqpRow {
                id: 5,
                parent: 3,
                detail: "SEARCH b USING INTEGER PRIMARY KEY (rowid=?)".into(),
            },
            EqpRow { id: 9, parent: 0, detail: "USE TEMP B-TREE FOR ORDER BY".into() },
        ];
        let plan = build_plan(&rows, "SELECT 1");
        let root = &plan["root"];
        assert_eq!(root["node_type"], "Query Plan");
        assert_eq!(root["children"].as_array().unwrap().len(), 2);
        assert_eq!(root["children"][0]["children"][0]["relation"], "b");
        assert_eq!(plan["original_query"], "SELECT 1");
        assert_eq!(plan["has_analyze_data"], false);
    }

    #[test]
    fn self_referencing_row_does_not_loop() {
        let rows = vec![EqpRow { id: 0, parent: 0, detail: "SCAN a".into() }];
        let plan = build_plan(&rows, "q");
        assert_eq!(plan["root"]["children"].as_array().unwrap().len(), 0);
    }
}
