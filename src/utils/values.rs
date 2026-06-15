use serde_json::Value;

/// Convert a JSON value to a form suitable for inclusion in the D1 REST API
/// `params` array (positional query parameters).  D1 accepts JSON scalars
/// directly.
pub fn to_d1_param(v: &Value) -> Value {
    match v {
        // JSON null maps to SQL NULL.
        Value::Null => Value::Null,
        // Booleans become 0/1 (SQLite has no native BOOLEAN type).
        Value::Bool(b) => Value::Number(if *b { 1.into() } else { 0.into() }),
        // Numbers and strings pass through unchanged.
        Value::Number(_) | Value::String(_) => v.clone(),
        // Objects and arrays are serialised to their JSON string representation
        // so they can be stored in TEXT columns.
        other => Value::String(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn null_passthrough() {
        assert_eq!(to_d1_param(&Value::Null), Value::Null);
    }

    #[test]
    fn bool_to_int() {
        assert_eq!(to_d1_param(&json!(true)), json!(1));
        assert_eq!(to_d1_param(&json!(false)), json!(0));
    }

    #[test]
    fn string_passthrough() {
        assert_eq!(to_d1_param(&json!("hello")), json!("hello"));
    }

    #[test]
    fn object_to_string() {
        let v = to_d1_param(&json!({"a": 1}));
        assert!(v.is_string());
        assert!(v.as_str().unwrap().contains("a"));
    }
}
