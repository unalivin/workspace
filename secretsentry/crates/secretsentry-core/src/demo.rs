use crate::allow::{EntrySpec, Kind};
use crate::fingerprint;

pub const ROOT: &str = "Demo: sample-shop";
pub const AUTHOR: &str = "Demo Author";
pub const TREE_DURATION_MS: i64 = 184;
pub const HISTORY_DURATION_MS: i64 = 412;
pub const TREE_FILES_SKIPPED: u64 = 2;
pub const ALLOWLIST_PATTERN: &str = "demo-fixtures/**";

const ALPHABET: &str = "q7Zp2LmX9vKc4RtB8nWd3FgH6jS1uYeOaIxNhTbVcMzQrPl5";

fn letters(offset: usize, length: usize) -> String {
    ALPHABET.chars().cycle().skip(offset % ALPHABET.len()).take(length).collect()
}

fn lower(offset: usize, length: usize) -> String {
    letters(offset, length).to_lowercase()
}

pub struct DemoFile {
    pub path: &'static str,
    pub content: String,
}

pub struct DemoCommit {
    pub id: String,
    pub subject: &'static str,
    pub days_ago: i64,
    pub files: Vec<DemoFile>,
}

fn file(path: &'static str, content: String) -> DemoFile {
    DemoFile { path, content }
}

fn settings() -> DemoFile {
    let key_id = ["AK", "IA", "Q4WZ7PLM2K6XTR3N"].concat();
    file(
        "config/settings.py",
        format!(
            "REGION = \"eu-west-1\"\nAWS_ACCESS_KEY_ID = \"{key_id}\"\naws_secret_access_key = \"{}\"\nDEBUG = False\n",
            lower(3, 40)
        ),
    )
}

fn release() -> DemoFile {
    file(
        "deploy/release.sh",
        format!("set -eu\nexport GITHUB_TOKEN={}{}\n./publish.sh\n", ["gh", "p_"].concat(), letters(11, 36)),
    )
}

pub fn allowlist_entry() -> EntrySpec {
    EntrySpec {
        kind: Kind::Path,
        pattern: ALLOWLIST_PATTERN.to_string(),
        note: "Demo: invented keys used by the payment tests".to_string(),
        ..EntrySpec::default()
    }
}

pub fn tree() -> Vec<DemoFile> {
    vec![
        settings(),
        release(),
        file(".npmrc", format!("//registry.npmjs.org/:_authToken={}{}\n", ["np", "m_"].concat(), letters(20, 36))),
        file(
            "docs/setup.md",
            format!("# Setup\n\nConnect with postgres://shop:{}@db.internal:5432/shop to load the sample orders.\n", lower(7, 14)),
        ),
        file(
            "src/notify.py",
            format!(
                "WEBHOOK = \"https://hooks.slack.com/services/T{}/B{}/{}\"\n",
                letters(1, 9).to_uppercase(),
                letters(15, 9).to_uppercase(),
                letters(27, 24)
            ),
        ),
        file(
            "demo-fixtures/payment_test.py",
            format!("STRIPE_KEY = \"{}{}\"\n", ["sk", "_live_"].concat(), letters(31, 28)),
        ),
        file("src/app.py", "def main():\n    return \"no secrets in this file\"\n".to_string()),
    ]
}

fn commit_id(number: usize) -> String {
    fingerprint::digest(format!("secretsentry demo commit {number}").as_bytes()).chars().take(40).collect()
}

pub fn history() -> Vec<DemoCommit> {
    vec![
        DemoCommit {
            id: commit_id(1),
            subject: "Add the deployment settings",
            days_ago: 40,
            files: vec![
                settings(),
                file(".env", format!("NPM_TOKEN={}{}\nLOG_LEVEL=info\n", ["np", "m_"].concat(), letters(40, 36))),
            ],
        },
        DemoCommit {
            id: commit_id(2),
            subject: "Remove the token from the environment file",
            days_ago: 33,
            files: vec![file(".env", "NPM_TOKEN=\nLOG_LEVEL=info\n".to_string())],
        },
        DemoCommit { id: commit_id(3), subject: "Add the release script", days_ago: 12, files: vec![release()] },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_demo_content_is_stable_and_labelled() {
        assert!(ROOT.starts_with("Demo: "));
        assert!(allowlist_entry().note.starts_with("Demo: "));
        let first: Vec<String> = tree().into_iter().map(|file| file.content).collect();
        let second: Vec<String> = tree().into_iter().map(|file| file.content).collect();
        assert_eq!(first, second);
        let commits = history();
        assert_eq!(commits.len(), 3);
        assert!(commits.iter().all(|commit| commit.id.len() == 40 && commit.id.bytes().all(|byte| byte.is_ascii_hexdigit())));
        assert!(commits.windows(2).all(|pair| pair[0].days_ago > pair[1].days_ago && pair[0].id != pair[1].id));
        assert_eq!(commits[0].files[0].content, tree()[0].content);
    }
}
