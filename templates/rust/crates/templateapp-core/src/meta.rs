pub const APP_NAME: &str = "Template App";
pub const SLUG: &str = "templateapp";
pub const APP_ID: &str = "io.github.unalivin.templateapp";
pub const ENV_PREFIX: &str = "TEMPLATEAPP";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const TAGLINE: &str = "TEMPLATE_TAGLINE";
pub const DESCRIPTION: &str = "TEMPLATE_DESCRIPTION";
pub const ICON_CONCEPT: &str = "TEMPLATE_ICON_CONCEPT";
pub const REPOSITORY_URL: &str = "https://github.com/unalivin/workspace";
pub const LICENSE: &str = "MIT";
pub const LICENSE_TEXT: &str = include_str!("../../../LICENSE");
pub const NOTICES_TEXT: &str = include_str!("../../../THIRD_PARTY_NOTICES.md");
pub const USAGE_TEXT: &str = include_str!("../../../docs/USAGE.md");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub profile: String,
    pub dashboard: bool,
}

pub fn build(dashboard: bool) -> BuildInfo {
    BuildInfo {
        version: VERSION.to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        profile: if cfg!(debug_assertions) { "debug".to_string() } else { "release".to_string() },
        dashboard,
    }
}

pub fn env_name(suffix: &str) -> String {
    let converted: String = suffix
        .chars()
        .map(|letter| if letter == '.' || letter == '-' { '_' } else { letter.to_ascii_uppercase() })
        .collect();
    format!("{ENV_PREFIX}_{converted}")
}

pub fn source_url() -> String {
    REPOSITORY_URL.to_string()
}

pub fn advisory_url() -> String {
    format!("{REPOSITORY_URL}/security/advisories/new")
}

pub fn encode_query(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

pub fn new_issue_url(title: &str, body: &str) -> String {
    format!("{REPOSITORY_URL}/issues/new?title={}&body={}", encode_query(title), encode_query(body))
}

pub fn is_repository_url(candidate: &str) -> bool {
    let Some(rest) = candidate.strip_prefix(REPOSITORY_URL) else {
        return false;
    };
    if !(rest.is_empty() || rest.starts_with('/') || rest.starts_with('?')) {
        return false;
    }
    if !candidate.is_ascii() || candidate.bytes().any(|byte| byte.is_ascii_control() || byte == b' ') {
        return false;
    }
    let path = rest.split(['?', '#']).next().unwrap_or("");
    let lowered = path.to_ascii_lowercase();
    !(path.contains("..")
        || path.contains("//")
        || path.contains('\\')
        || lowered.contains("%2f")
        || lowered.contains("%5c")
        || lowered.contains("%2e"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_complete() {
        assert!(!APP_NAME.is_empty() && !SLUG.is_empty() && !VERSION.is_empty());
        assert!(APP_ID.ends_with(SLUG));
        assert!(SLUG.chars().all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-'));
        assert!(LICENSE_TEXT.contains("MIT License"));
        assert!(USAGE_TEXT.contains("Getting started"));
    }

    #[test]
    fn environment_names_follow_the_setting_keys() {
        assert_eq!(env_name("appearance.density"), format!("{ENV_PREFIX}_APPEARANCE_DENSITY"));
        assert_eq!(env_name("config-dir"), format!("{ENV_PREFIX}_CONFIG_DIR"));
    }

    #[test]
    fn only_links_below_the_repository_are_accepted() {
        let accepted = [source_url(), advisory_url(), new_issue_url("A title", "Line one\nLine two & more")];
        for link in accepted {
            assert!(is_repository_url(&link), "{link}");
        }
        let base = REPOSITORY_URL;
        let rejected = [
            "https://example.test/unalivin/workspace".to_string(),
            format!("{base}-other"),
            format!("{base}/../other"),
            format!("{base}//issues"),
            format!("{base}/%2e%2e/other"),
            format!("{base}/a%2Fb"),
            format!("{base}/a\\b"),
            format!("{base}/a b"),
            base.replace("https://", "http://"),
            base.replace("https://", "https://user@"),
            String::new(),
        ];
        for link in rejected {
            assert!(!is_repository_url(&link), "{link}");
        }
    }

    #[test]
    fn issue_links_encode_every_reserved_character() {
        let link = new_issue_url("a b&c", "x=y\n#z");
        assert!(link.ends_with("/issues/new?title=a%20b%26c&body=x%3Dy%0A%23z"), "{link}");
    }
}
