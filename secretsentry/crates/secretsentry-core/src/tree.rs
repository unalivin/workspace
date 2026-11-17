use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{AppError, Result};

pub const BINARY_PROBE_BYTES: usize = 8000;
pub const MAX_DEPTH: usize = 64;
pub const MAX_FILES: u64 = 1_000_000;
pub const DEFAULT_MAX_FILE_BYTES: u64 = 1024 * 1024;
pub const DEFAULT_SKIPPED_DIRECTORIES: &[&str] = &[".git", "node_modules", "target", ".venv", "__pycache__"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub max_file_bytes: u64,
    pub skipped_directories: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            skipped_directories: DEFAULT_SKIPPED_DIRECTORIES.iter().map(|name| (*name).to_string()).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    pub files_scanned: u64,
    pub bytes_scanned: u64,
    pub binary: u64,
    pub too_large: u64,
    pub links: u64,
    pub special: u64,
    pub unreadable: u64,
    pub skipped_directories: u64,
    pub too_deep: u64,
    pub cancelled: bool,
    pub file_limit_reached: bool,
}

impl Stats {
    pub fn skipped(&self) -> u64 {
        self.binary
            + self.too_large
            + self.links
            + self.special
            + self.unreadable
            + self.skipped_directories
            + self.too_deep
    }

    pub fn complete(&self) -> bool {
        !self.cancelled && !self.file_limit_reached
    }
}

struct Walk<'a> {
    options: &'a Options,
    cancel: &'a AtomicBool,
    visit: &'a mut dyn FnMut(&str, &str),
    stats: Stats,
}

impl Walk<'_> {
    fn stopped(&mut self) -> bool {
        if self.cancel.load(Ordering::Relaxed) {
            self.stats.cancelled = true;
        }
        self.stats.cancelled || self.stats.file_limit_reached
    }

    fn file(&mut self, path: &Path, relative: &str, size: u64) {
        let limit = self.options.max_file_bytes;
        if size > limit {
            self.stats.too_large += 1;
            return;
        }
        let mut bytes = Vec::new();
        let read = fs::File::open(path).and_then(|file| file.take(limit.saturating_add(1)).read_to_end(&mut bytes));
        if read.is_err() {
            self.stats.unreadable += 1;
            return;
        }
        if bytes.len() as u64 > limit {
            self.stats.too_large += 1;
            return;
        }
        if bytes.iter().take(BINARY_PROBE_BYTES).any(|byte| *byte == 0) {
            self.stats.binary += 1;
            return;
        }
        self.stats.files_scanned += 1;
        self.stats.bytes_scanned += bytes.len() as u64;
        (self.visit)(relative, &String::from_utf8_lossy(&bytes));
        if self.stats.files_scanned >= MAX_FILES {
            self.stats.file_limit_reached = true;
        }
    }

    fn directory(&mut self, directory: &Path, relative: &str, depth: usize) -> std::io::Result<()> {
        let mut names = Vec::new();
        for entry in fs::read_dir(directory)? {
            match entry {
                Ok(entry) => names.push(entry.file_name()),
                Err(_) => self.stats.unreadable += 1,
            }
        }
        names.sort();
        for name in names {
            if self.stopped() {
                return Ok(());
            }
            let path = directory.join(&name);
            let shown = name.to_string_lossy();
            let child = if relative.is_empty() { shown.to_string() } else { format!("{relative}/{shown}") };
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                self.stats.unreadable += 1;
                continue;
            };
            let kind = metadata.file_type();
            if kind.is_symlink() {
                self.stats.links += 1;
            } else if kind.is_dir() {
                if self.options.skipped_directories.iter().any(|skipped| skipped.as_str() == shown.as_ref()) {
                    self.stats.skipped_directories += 1;
                } else if depth >= MAX_DEPTH {
                    self.stats.too_deep += 1;
                } else if self.directory(&path, &child, depth + 1).is_err() {
                    self.stats.unreadable += 1;
                }
            } else if kind.is_file() {
                self.file(&path, &child, metadata.len());
            } else {
                self.stats.special += 1;
            }
        }
        Ok(())
    }
}

pub fn check_root(root: &Path) -> Result<()> {
    match fs::metadata(root) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(AppError::invalid(
            "path",
            format!("\"{}\" is not a directory", root.display()),
            "Name the directory that contains the files to scan",
        )),
        Err(error) => Err(AppError::invalid(
            "path",
            format!("\"{}\" cannot be opened ({error})", root.display()),
            "Check the spelling and the permissions of the directory",
        )),
    }
}

pub fn walk(root: &Path, options: &Options, cancel: &AtomicBool, visit: &mut dyn FnMut(&str, &str)) -> Result<Stats> {
    check_root(root)?;
    let mut walk = Walk { options, cancel, visit, stats: Stats::default() };
    if walk.stopped() {
        return Ok(walk.stats);
    }
    walk.directory(root, "", 1)
        .map_err(|error| AppError::Io(format!("the directory \"{}\" cannot be read: {error}", root.display())))?;
    Ok(walk.stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::TempDir;

    fn write(root: &Path, relative: &str, content: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn never(_: &str, _: &str) {
        panic!("nothing is read");
    }

    fn collect(root: &Path, options: &Options) -> (Vec<(String, String)>, Stats) {
        let mut seen = Vec::new();
        let stats = walk(root, options, &AtomicBool::new(false), &mut |path, content| {
            seen.push((path.to_string(), content.to_string()));
        })
        .unwrap();
        (seen, stats)
    }

    #[test]
    fn files_are_visited_in_a_stable_order_and_skips_are_counted() {
        let directory = TempDir::new("tree-walk");
        let root = directory.path();
        write(root, "b.txt", b"second");
        write(root, "a.txt", b"first");
        write(root, "sub/deep/c.txt", b"third");
        write(root, "sub/b.txt", b"nested");
        write(root, "image.bin", b"PNG\0data");
        write(root, "large.txt", &[b'x'; 65]);
        write(root, "exact.txt", &[b'y'; 64]);
        write(root, ".git/config", b"ignored");
        write(root, "node_modules/pkg/index.js", b"ignored");
        write(root, "sub/target/out.txt", b"ignored");
        write(root, "latin.txt", &[b'c', b'a', b'f', 0xe9]);
        let options = Options { max_file_bytes: 64, ..Options::default() };
        let (seen, stats) = collect(root, &options);
        let paths: Vec<&str> = seen.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(paths, vec!["a.txt", "b.txt", "exact.txt", "latin.txt", "sub/b.txt", "sub/deep/c.txt"]);
        assert_eq!(seen[0].1, "first");
        assert_eq!(seen[3].1, "caf\u{fffd}");
        assert_eq!(stats.files_scanned, 6);
        assert_eq!(stats.bytes_scanned, 5 + 6 + 64 + 4 + 6 + 5);
        assert_eq!((stats.binary, stats.too_large, stats.skipped_directories), (1, 1, 3));
        assert_eq!(stats.skipped(), 5);
        assert!(stats.complete());
        assert_eq!(collect(root, &options), (seen, stats));
    }

    #[test]
    fn the_skipped_directories_are_a_choice() {
        let directory = TempDir::new("tree-skip");
        let root = directory.path();
        write(root, "node_modules/pkg/index.js", b"code");
        write(root, "vendor/lib.go", b"code");
        let none = Options { skipped_directories: Vec::new(), ..Options::default() };
        assert_eq!(collect(root, &none).0.len(), 2);
        let vendor = Options { skipped_directories: vec!["vendor".to_string()], ..Options::default() };
        let (seen, stats) = collect(root, &vendor);
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "node_modules/pkg/index.js");
        assert_eq!(stats.skipped_directories, 1);
    }

    #[test]
    fn a_cancelled_walk_stops_and_says_so() {
        let directory = TempDir::new("tree-cancel");
        let root = directory.path();
        for number in 0..5 {
            write(root, &format!("file{number}.txt"), b"text");
        }
        let cancel = AtomicBool::new(false);
        let mut seen = 0;
        let stats = walk(root, &Options::default(), &cancel, &mut |_, _| {
            seen += 1;
            cancel.store(true, Ordering::Relaxed);
        })
        .unwrap();
        assert_eq!(seen, 1);
        assert!(stats.cancelled && !stats.complete());
        assert_eq!(stats.files_scanned, 1);

        let before = walk(root, &Options::default(), &AtomicBool::new(true), &mut never).unwrap();
        assert!(before.cancelled);
        assert_eq!(before.files_scanned, 0);
    }

    #[test]
    fn a_root_that_is_not_a_directory_is_refused_with_the_fix() {
        let directory = TempDir::new("tree-root");
        write(directory.path(), "file.txt", b"text");
        let file = walk(&directory.path().join("file.txt"), &Options::default(), &AtomicBool::new(false), &mut never)
            .unwrap_err()
            .to_string();
        assert!(file.starts_with("path: ") && file.contains("is not a directory"), "{file}");
        let missing = walk(&directory.path().join("missing"), &Options::default(), &AtomicBool::new(false), &mut never)
            .unwrap_err()
            .to_string();
        assert!(missing.starts_with("path: ") && missing.contains("cannot be opened"), "{missing}");
    }

    #[test]
    fn an_empty_directory_gives_empty_statistics() {
        let directory = TempDir::new("tree-empty");
        let (seen, stats) = collect(directory.path(), &Options::default());
        assert!(seen.is_empty());
        assert_eq!(stats, Stats::default());
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_counted_and_never_followed() {
        let outside = TempDir::new("tree-outside");
        write(outside.path(), "secret.txt", b"outside the scope");
        let directory = TempDir::new("tree-links");
        let root = directory.path();
        write(root, "inside.txt", b"inside");
        std::os::unix::fs::symlink(outside.path(), root.join("escape")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), root.join("alias.txt")).unwrap();
        std::os::unix::fs::symlink(root, root.join("loop")).unwrap();
        let (seen, stats) = collect(root, &Options::default());
        assert_eq!(seen, vec![("inside.txt".to_string(), "inside".to_string())]);
        assert_eq!(stats.links, 3);
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_directory_is_counted_and_the_rest_is_scanned() {
        use std::os::unix::fs::PermissionsExt;
        let directory = TempDir::new("tree-denied");
        let root = directory.path();
        write(root, "open/a.txt", b"readable");
        write(root, "closed/b.txt", b"hidden");
        let closed = root.join("closed");
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o000)).unwrap();
        let can_still_read = fs::read_dir(&closed).is_ok();
        let (seen, stats) = collect(root, &Options::default());
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o700)).unwrap();
        if can_still_read {
            return;
        }
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "open/a.txt");
        assert_eq!(stats.unreadable, 1);
    }

    #[test]
    fn directories_deeper_than_the_limit_are_counted_not_entered() {
        let directory = TempDir::new("tree-deep");
        let mut path = directory.path().to_path_buf();
        for _ in 0..MAX_DEPTH {
            path.push("d");
        }
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("too-deep.txt"), b"text").unwrap();
        fs::write(path.parent().unwrap().join("reachable.txt"), b"text").unwrap();
        let (seen, stats) = collect(directory.path(), &Options::default());
        assert_eq!(seen.len(), 1);
        assert!(seen[0].0.ends_with("/reachable.txt"));
        assert_eq!(stats.too_deep, 1);
    }
}
