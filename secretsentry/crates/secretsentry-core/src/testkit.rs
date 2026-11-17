use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::clock::Timestamp;

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let number = COUNTER.fetch_add(1, Ordering::SeqCst);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let base = fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
        let path = base.join(format!("secretsentry-test-{label}-{}-{number}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).expect("create a temporary directory");
        TempDir { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn fixed_time() -> Timestamp {
    Timestamp::from_civil(2026, 3, 2, 9, 30, 0).expect("a valid fixed time")
}

pub fn sample_letters(length: usize) -> String {
    "q7Zp2LmX9vKc4RtB8nWd3FgH6jS1uYeOaIxNhTbVcMzQrPl5".chars().cycle().take(length).collect()
}

pub fn sample_cloud_key() -> String {
    ["AK", "IA", "Q4WZ7PLM2K6XTR3N"].concat()
}

pub fn sample_hosting_token() -> String {
    ["gh", "p_", &sample_letters(36)].concat()
}

pub fn sample_package_token() -> String {
    ["np", "m_", &sample_letters(36)].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_directories_are_unique_and_removed() {
        let first = TempDir::new("kit");
        let second = TempDir::new("kit");
        assert_ne!(first.path(), second.path());
        let kept = first.path().to_path_buf();
        fs::write(kept.join("file"), "x").unwrap();
        drop(first);
        assert!(!kept.exists());
        assert!(second.path().is_dir());
        assert_eq!(fixed_time().to_rfc3339(), "2026-03-02T09:30:00Z");
        assert_eq!(sample_letters(50).len(), 50);
        assert_eq!((sample_cloud_key().len(), sample_hosting_token().len(), sample_package_token().len()), (20, 40, 40));
    }
}
