use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::files;
use crate::fingerprint;

pub const FORMAT: &str = "secretsentry-baseline";
pub const VERSION: u32 = 1;
pub const MAX_FILE_BYTES: u64 = 64 << 20;
pub const MAX_ENTRIES: usize = 500_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format: String,
    version: u32,
    key: String,
    entries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baseline {
    key: Vec<u8>,
    entries: BTreeSet<String>,
}

fn refused(problem: impl Into<String>) -> AppError {
    AppError::invalid("baseline", problem, "Write the baseline file again with: secretsentry scan <directory> --write-baseline <file>")
}

impl Baseline {
    pub fn fresh() -> Result<Baseline> {
        Ok(Baseline { key: fingerprint::new_key()?, entries: BTreeSet::new() })
    }

    pub fn with_key(key: &[u8]) -> Baseline {
        Baseline { key: key.to_vec(), entries: BTreeSet::new() }
    }

    pub fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn entries(&self) -> &BTreeSet<String> {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, print: &str) -> bool {
        self.entries.contains(print)
    }

    pub fn replace_entries(&mut self, entries: impl IntoIterator<Item = String>) -> Result<()> {
        let entries: BTreeSet<String> = entries.into_iter().collect();
        if entries.len() > MAX_ENTRIES {
            return Err(refused(format!("{} findings are more than a baseline file can hold ({MAX_ENTRIES})", entries.len())));
        }
        if let Some(wrong) = entries.iter().find(|entry| !fingerprint::is_fingerprint(entry)) {
            return Err(refused(format!("\"{}\" is not a fingerprint", wrong.chars().take(80).collect::<String>())));
        }
        self.entries = entries;
        Ok(())
    }

    pub fn parse(text: &str) -> Result<Baseline> {
        let document: Document =
            serde_json::from_str(text).map_err(|error| refused(format!("the file is not a baseline file ({error})")))?;
        if document.format != FORMAT {
            return Err(refused("the file is not a baseline file (its format name is different)"));
        }
        if document.version != VERSION {
            return Err(AppError::invalid(
                "baseline",
                format!("the file has format version {}, and this version reads version {VERSION}", document.version),
                "Write the baseline file again with this version of the program",
            ));
        }
        let key = fingerprint::key_from_hex(&document.key).ok_or_else(|| refused("the key of the file is not usable"))?;
        let mut baseline = Baseline { key, entries: BTreeSet::new() };
        baseline.replace_entries(document.entries)?;
        Ok(baseline)
    }

    pub fn render(&self) -> Result<String> {
        let document = Document {
            format: FORMAT.to_string(),
            version: VERSION,
            key: fingerprint::to_hex(&self.key),
            entries: self.entries.iter().cloned().collect(),
        };
        Ok(serde_json::to_string_pretty(&document)? + "\n")
    }

    pub fn load(path: &Path) -> Result<Baseline> {
        let raw = files::read_limited(path, MAX_FILE_BYTES)?;
        let text = String::from_utf8(raw).map_err(|_| refused("the file is not text"))?;
        Baseline::parse(&text)
    }

    pub fn save(&self, path: &Path, overwrite: bool) -> Result<()> {
        files::write_private(path, self.render()?.as_bytes(), overwrite)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::testkit::TempDir;

    fn print(seed: &str) -> String {
        fingerprint::digest(seed.as_bytes())
    }

    #[test]
    fn a_baseline_round_trips_through_its_file() {
        let directory = TempDir::new("baseline-file");
        let path = directory.path().join("nested").join("baseline.json");
        let mut baseline = Baseline::with_key(&[7u8; 32]);
        assert!(baseline.is_empty());
        baseline.replace_entries([print("b"), print("a"), print("b")]).unwrap();
        assert_eq!(baseline.len(), 2);
        assert!(baseline.contains(&print("a")) && !baseline.contains(&print("c")));
        baseline.save(&path, false).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("{\n  \"format\": \"secretsentry-baseline\",\n  \"version\": 1,\n  \"key\": \"0707"), "{text}");
        assert!(text.ends_with("]\n}\n"));
        assert_eq!(Baseline::load(&path).unwrap(), baseline);
        assert_eq!(Baseline::load(&path).unwrap().key(), &[7u8; 32]);
        let kept = baseline.save(&path, false).unwrap_err().to_string();
        assert!(kept.contains("already exists and was not changed"), "{kept}");
        baseline.replace_entries([]).unwrap();
        baseline.save(&path, true).unwrap();
        assert!(Baseline::load(&path).unwrap().is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn fresh_baselines_have_different_keys() {
        let (first, second) = (Baseline::fresh().unwrap(), Baseline::fresh().unwrap());
        assert_eq!(first.key().len(), fingerprint::KEY_LENGTH);
        assert_ne!(first.key(), second.key());
    }

    #[test]
    fn other_files_are_refused_with_a_reason() {
        let key = "ab".repeat(32);
        let cases = [
            ("", "the file is not a baseline file ("),
            ("[]", "the file is not a baseline file ("),
            ("{\"format\":\"secretsentry-baseline\",\"version\":1,\"key\":\"\",\"entries\":[],\"more\":1}", "the file is not a baseline file ("),
            (&format!("{{\"format\":\"other\",\"version\":1,\"key\":\"{key}\",\"entries\":[]}}"), "its format name is different"),
            (&format!("{{\"format\":\"secretsentry-baseline\",\"version\":2,\"key\":\"{key}\",\"entries\":[]}}"), "the file has format version 2, and this version reads version 1"),
            ("{\"format\":\"secretsentry-baseline\",\"version\":1,\"key\":\"abc\",\"entries\":[]}", "the key of the file is not usable"),
            (&format!("{{\"format\":\"secretsentry-baseline\",\"version\":1,\"key\":\"{key}\",\"entries\":[\"zz\"]}}"), "\"zz\" is not a fingerprint"),
        ];
        for (text, expected) in cases {
            let problem = Baseline::parse(text).unwrap_err().to_string();
            assert!(problem.starts_with("baseline: ") && problem.contains(expected), "{text} gave {problem}");
        }
        let directory = TempDir::new("baseline-binary");
        let path = directory.path().join("baseline.json");
        std::fs::write(&path, [0xffu8, 0xfe, 0x00]).unwrap();
        assert!(Baseline::load(&path).unwrap_err().to_string().contains("the file is not text"));
        assert!(Baseline::load(&directory.path().join("absent.json")).unwrap_err().to_string().starts_with("cannot read "));
    }

    proptest! {
        #[test]
        fn any_text_is_parsed_or_refused(text in "\\PC{0,200}") {
            if let Ok(baseline) = Baseline::parse(&text) {
                prop_assert_eq!(Baseline::parse(&baseline.render().unwrap()).unwrap(), baseline);
            }
        }

        #[test]
        fn any_set_of_fingerprints_round_trips(seeds in proptest::collection::vec("[a-z]{1,8}", 0..20), key in proptest::collection::vec(any::<u8>(), 32)) {
            let mut baseline = Baseline::with_key(&key);
            baseline.replace_entries(seeds.iter().map(|seed| print(seed))).unwrap();
            prop_assert_eq!(Baseline::parse(&baseline.render().unwrap()).unwrap(), baseline);
        }
    }
}
