use crate::ingest::IngestError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledQuery {
    pub fts_match: String,
    pub lower_timestamp: Option<String>,
    pub upper_timestamp: Option<String>,
    pub severity: Option<String>,
    pub host: Option<String>,
}

pub fn compile_query(input: &str) -> Result<CompiledQuery, IngestError> {
    let mut result = CompiledQuery {
        fts_match: String::new(),
        lower_timestamp: None,
        upper_timestamp: None,
        severity: None,
        host: None,
    };
    let mut terms = Vec::new();
    let mut chars = input.chars().peekable();
    while chars.peek().is_some() {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }
        let token = if chars.peek() == Some(&'"') {
            chars.next();
            let mut quoted = String::new();
            let mut closed = false;
            while let Some(c) = chars.next() {
                if c == '"' {
                    closed = true;
                    break;
                }
                if c == '\\' {
                    quoted.push(chars.next().ok_or_else(|| {
                        IngestError::Malformed("unfinished escape in quoted phrase".into())
                    })?);
                } else {
                    quoted.push(c);
                }
            }
            if !closed {
                return Err(IngestError::Malformed("unterminated quoted phrase".into()));
            }
            format!("\"{}\"", quoted.replace('"', ""))
        } else {
            let mut value = String::new();
            while chars.peek().is_some_and(|c| !c.is_whitespace()) {
                value.push(chars.next().unwrap());
            }
            value
        };
        if token.eq_ignore_ascii_case("AND")
            || token.eq_ignore_ascii_case("OR")
            || token.eq_ignore_ascii_case("NOT")
        {
            terms.push(token.to_ascii_uppercase());
        } else if let Some(v) = token.strip_prefix("severity:") {
            result.severity = Some(v.to_ascii_lowercase());
        } else if let Some(v) = token.strip_prefix("host:") {
            result.host = Some(v.to_string());
        } else if let Some(v) = token.strip_prefix("ts>=") {
            result.lower_timestamp = Some(v.to_string());
        } else if let Some(v) = token.strip_prefix("ts<=") {
            result.upper_timestamp = Some(v.to_string());
        } else if let Some((field, value)) = token.split_once(':') {
            if field.is_empty() || value.is_empty() {
                return Err(IngestError::Malformed(
                    "field query requires field:value".into(),
                ));
            }
            terms.push(format!(
                "{}:{}",
                field
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect::<String>(),
                value.replace('"', "")
            ));
        } else {
            terms.push(format!("\"{}\"", token.replace('"', "")));
        }
    }
    if terms.iter().any(|term| term.is_empty()) {
        return Err(IngestError::Malformed("empty query term".into()));
    }
    result.fts_match = terms.join(" ");
    Ok(result)
}

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
