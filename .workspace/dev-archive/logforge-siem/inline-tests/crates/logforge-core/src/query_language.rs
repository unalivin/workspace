#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_fields_phrases_operators_and_time_bounds() {
        let query =
            compile_query("host:web severity:error ts>=2026-01-01 \"login denied\" AND token")
                .unwrap();
        assert_eq!(query.host.as_deref(), Some("web"));
        assert_eq!(query.severity.as_deref(), Some("error"));
        assert_eq!(query.lower_timestamp.as_deref(), Some("2026-01-01"));
        assert_eq!(query.fts_match, "\"login denied\" AND \"token\"");
    }

    #[test]
    fn rejects_unterminated_phrases_and_escapes() {
        assert!(compile_query("\"open phrase").is_err());
        assert!(compile_query("word\\").is_err());
    }
}
