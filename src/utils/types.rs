/// Normalise a raw SQLite/D1 column type string to a canonical upper-case form
/// that Tabularis can display and compare consistently.
pub fn normalize(raw: &str) -> String {
    let upper = raw.trim().to_uppercase();
    // Strip any trailing length/precision, e.g. VARCHAR(255) → VARCHAR
    let base = upper
        .split('(')
        .next()
        .unwrap_or(&upper)
        .trim()
        .to_string();

    // Map common SQLite affinity aliases to their canonical names.
    match base.as_str() {
        "INT" | "INT2" | "INT8" | "INTEGER" | "TINYINT" | "SMALLINT" | "MEDIUMINT"
        | "BIGINT" | "UNSIGNED BIG INT" => "INTEGER".into(),
        "CLOB" | "CHARACTER" | "NATIVE CHARACTER" | "NCHAR" | "NVARCHAR" | "VARYING CHARACTER"
        | "VARCHAR" | "TEXT" => "TEXT".into(),
        "FLOAT" | "DOUBLE" | "DOUBLE PRECISION" | "REAL" => "REAL".into(),
        "DECIMAL" | "NUMERIC" | "BOOLEAN" | "DATE" | "DATETIME" | "TIMESTAMP" => base,
        "" => "TEXT".into(), // D1 allows columns without an explicit type
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_variants() {
        assert_eq!(normalize("INT"), "INTEGER");
        assert_eq!(normalize("bigint"), "INTEGER");
        assert_eq!(normalize("TINYINT"), "INTEGER");
    }

    #[test]
    fn text_variants() {
        assert_eq!(normalize("varchar"), "TEXT");
        assert_eq!(normalize("VARCHAR(255)"), "TEXT");
        assert_eq!(normalize("nvarchar(100)"), "TEXT");
    }

    #[test]
    fn float_variants() {
        assert_eq!(normalize("FLOAT"), "REAL");
        assert_eq!(normalize("double precision"), "REAL");
    }

    #[test]
    fn passthrough() {
        assert_eq!(normalize("BLOB"), "BLOB");
        assert_eq!(normalize("JSON"), "JSON");
        assert_eq!(normalize("DATE"), "DATE");
    }

    #[test]
    fn empty_type() {
        assert_eq!(normalize(""), "TEXT");
    }
}
