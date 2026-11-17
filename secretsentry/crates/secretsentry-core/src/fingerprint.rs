use sha2::{Digest, Sha256};

use crate::error::{AppError, Result};

pub const KEY_LENGTH: usize = 32;
pub const LENGTH: usize = 64;

const FINDING_DOMAIN: &[u8] = b"secretsentry.finding.v1";
const VALUE_DOMAIN: &[u8] = b"secretsentry.value.v1";

fn field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

pub fn to_hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

fn finish(hasher: Sha256) -> String {
    let digest: [u8; 32] = hasher.finalize().into();
    to_hex(&digest)
}

pub fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    finish(hasher)
}

pub fn finding(key: &[u8], rule: &str, path: &str, secret: &str) -> String {
    let mut hasher = Sha256::new();
    field(&mut hasher, FINDING_DOMAIN);
    field(&mut hasher, key);
    field(&mut hasher, rule.as_bytes());
    field(&mut hasher, path.as_bytes());
    field(&mut hasher, secret.as_bytes());
    finish(hasher)
}

pub fn value(key: &[u8], secret: &str) -> String {
    let mut hasher = Sha256::new();
    field(&mut hasher, VALUE_DOMAIN);
    field(&mut hasher, key);
    field(&mut hasher, secret.as_bytes());
    finish(hasher)
}

pub fn is_fingerprint(text: &str) -> bool {
    text.len() == LENGTH && text.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn new_key() -> Result<Vec<u8>> {
    let mut key = vec![0u8; KEY_LENGTH];
    getrandom::fill(&mut key).map_err(|error| AppError::Io(format!("the system random source failed: {error}")))?;
    Ok(key)
}

pub fn key_from_hex(text: &str) -> Option<Vec<u8>> {
    if !is_fingerprint(text) {
        return None;
    }
    let digits: Vec<u8> = text.bytes().map(|byte| if byte.is_ascii_digit() { byte - b'0' } else { byte - b'a' + 10 }).collect();
    Some(digits.chunks_exact(2).map(|pair| pair[0] * 16 + pair[1]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn the_digest_matches_the_published_sha256_vectors() {
        assert_eq!(digest(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(digest(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn fingerprints_depend_on_every_part_and_never_show_the_value() {
        let key = [7u8; KEY_LENGTH];
        let base = finding(&key, "rule-a", "src/app.env", "value-one");
        assert!(is_fingerprint(&base));
        assert_eq!(base, finding(&key, "rule-a", "src/app.env", "value-one"));
        assert_ne!(base, finding(&[8u8; KEY_LENGTH], "rule-a", "src/app.env", "value-one"));
        assert_ne!(base, finding(&key, "rule-b", "src/app.env", "value-one"));
        assert_ne!(base, finding(&key, "rule-a", "src/other.env", "value-one"));
        assert_ne!(base, finding(&key, "rule-a", "src/app.env", "value-two"));
        assert_ne!(base, value(&key, "value-one"));
        assert!(!base.contains("value-one"));
    }

    #[test]
    fn field_boundaries_cannot_be_shifted() {
        let key = [1u8; KEY_LENGTH];
        assert_ne!(finding(&key, "ab", "c", "d"), finding(&key, "a", "bc", "d"));
        assert_ne!(finding(&key, "a", "bc", "d"), finding(&key, "a", "b", "cd"));
    }

    #[test]
    fn keys_are_random_and_survive_the_text_form() {
        let first = new_key().unwrap();
        let second = new_key().unwrap();
        assert_eq!(first.len(), KEY_LENGTH);
        assert_ne!(first, second);
        assert_eq!(key_from_hex(&to_hex(&first)), Some(first));
        assert_eq!(key_from_hex("abc"), None);
        assert_eq!(key_from_hex(&"G".repeat(64)), None);
        assert_eq!(key_from_hex(&"A".repeat(64)), None);
        assert!(!is_fingerprint(&"é".repeat(32)));
    }

    proptest! {
        #[test]
        fn hexadecimal_text_is_read_back(bytes in proptest::collection::vec(any::<u8>(), 32)) {
            prop_assert_eq!(key_from_hex(&to_hex(&bytes)), Some(bytes));
        }

        #[test]
        fn any_text_is_accepted_or_refused_without_a_panic(text in ".{0,80}") {
            let _ = key_from_hex(&text);
        }
    }
}
