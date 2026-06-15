/// Wraps an identifier in ANSI double-quotes, escaping any embedded
/// double-quotes by doubling them.  D1 uses SQLite quoting rules.
pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        assert_eq!(quote_ident("users"), "\"users\"");
    }

    #[test]
    fn embedded_quote() {
        assert_eq!(quote_ident("my\"table"), "\"my\"\"table\"");
    }

    #[test]
    fn spaces() {
        assert_eq!(quote_ident("my table"), "\"my table\"");
    }
}
