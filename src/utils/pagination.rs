/// Returns true if the SQL statement is a query that produces a result set
/// (SELECT, WITH…SELECT, VALUES).
pub fn is_select(sql: &str) -> bool {
    let s = sql.trim_start().to_ascii_uppercase();
    s.starts_with("SELECT") || s.starts_with("WITH") || s.starts_with("VALUES")
}

/// Returns true if the SQL already contains a LIMIT clause (heuristic —
/// sufficient for deciding whether to inject pagination).
pub fn has_limit(sql: &str) -> bool {
    sql.to_ascii_uppercase().contains(" LIMIT ")
}

/// Strip a trailing semicolon from SQL so it can be safely wrapped or
/// extended.
pub fn strip_trailing_semicolon(sql: &str) -> &str {
    sql.trim_end().trim_end_matches(';').trim_end()
}

/// Build a paginated SQL string and a corresponding COUNT(*) wrapper.
/// The caller should run both via the D1 batch endpoint.
pub fn paginated_pair(sql: &str, page: u32, page_size: u32) -> (String, String) {
    let clean = strip_trailing_semicolon(sql);
    let offset = (page.saturating_sub(1) as u64) * (page_size as u64);
    let data_sql = format!("{} LIMIT {} OFFSET {}", clean, page_size, offset);
    let count_sql = format!("SELECT COUNT(*) AS _count FROM ({}) AS _q", clean);
    (data_sql, count_sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_detection() {
        assert!(is_select("SELECT 1"));
        assert!(is_select("  select * from t"));
        assert!(is_select("WITH cte AS (SELECT 1) SELECT * FROM cte"));
        assert!(is_select("VALUES (1, 2)"));
        assert!(!is_select("INSERT INTO t VALUES (1)"));
        assert!(!is_select("UPDATE t SET a = 1"));
        assert!(!is_select("DELETE FROM t"));
    }

    #[test]
    fn limit_detection() {
        assert!(has_limit("SELECT * FROM t LIMIT 10"));
        assert!(has_limit("SELECT * FROM t limit 10 OFFSET 5"));
        assert!(!has_limit("SELECT * FROM t"));
    }

    #[test]
    fn strip_semicolon() {
        assert_eq!(strip_trailing_semicolon("SELECT 1;"), "SELECT 1");
        assert_eq!(strip_trailing_semicolon("SELECT 1;  "), "SELECT 1");
        assert_eq!(strip_trailing_semicolon("SELECT 1"), "SELECT 1");
    }

    #[test]
    fn pagination_pair_first_page() {
        let (data, count) = paginated_pair("SELECT * FROM t", 1, 100);
        assert!(data.contains("LIMIT 100 OFFSET 0"));
        assert!(count.contains("COUNT(*)"));
        assert!(count.contains("SELECT * FROM t"));
    }

    #[test]
    fn pagination_pair_second_page() {
        let (data, _) = paginated_pair("SELECT * FROM t", 2, 50);
        assert!(data.contains("LIMIT 50 OFFSET 50"));
    }
}
