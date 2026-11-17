#![no_main]

use app_core::git::{self, ObjectId, PackIndex};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok((_, body)) = git::split_loose(data) {
        assert!(body.len() < data.len());
    }
    if let Ok(commit) = git::parse_commit(data) {
        assert!(commit.parents.len() <= git::MAX_PARENTS);
        assert!(!commit.subject.chars().any(char::is_control));
        assert!(!commit.author.chars().any(char::is_control));
    }
    let _ = git::parse_tag(data);
    if let Ok(entries) = git::parse_tree(data) {
        for entry in entries {
            assert!(!entry.name.is_empty() && !entry.name.contains('/'));
        }
    }
    if let Ok(index) = PackIndex::parse(data.to_vec()) {
        let probe = [data.last().copied().unwrap_or(0); git::ID_LENGTH];
        if let Some(id) = ObjectId::from_bytes(&probe) {
            let _ = index.find(&id);
        }
    }
});
