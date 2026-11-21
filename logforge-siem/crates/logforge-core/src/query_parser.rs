#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedQuery {
    pub fts_terms: Vec<String>,
    pub severity_filter: Option<String>,
    pub host_filter: Option<String>,
}

pub fn parse_search_query(raw: &str) -> ParsedQuery {
    let mut fts_terms = Vec::new();
    let mut severity_filter = None;
    let mut host_filter = None;

    for token in raw.split_whitespace() {
        if let Some(val) = token.strip_prefix("severity:") {
            if !val.is_empty() {
                severity_filter = Some(val.to_ascii_lowercase());
            }
        } else if let Some(val) = token.strip_prefix("host:") {
            if !val.is_empty() {
                host_filter = Some(val.to_string());
            }
        } else {
            let clean: String = token
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if !clean.is_empty() {
                fts_terms.push(clean);
            }
        }
    }

    ParsedQuery {
        fts_terms,
        severity_filter,
        host_filter,
    }
}
