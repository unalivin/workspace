use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{AppError, Result};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn restrict(path: &Path, directory: bool) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if directory { 0o700 } else { 0o600 };
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|error| AppError::Io(format!("cannot restrict \"{}\": {error}", path.display())))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, directory);
    }
    Ok(())
}

pub fn create_private_dir(path: &Path) -> Result<()> {
    if !path.is_dir() {
        fs::create_dir_all(path)
            .map_err(|error| AppError::Io(format!("cannot create the directory \"{}\": {error}", path.display())))?;
        restrict(path, true)?;
    }
    Ok(())
}

fn temporary_sibling(path: &Path) -> PathBuf {
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| "file".to_string());
    let number = SEQUENCE.fetch_add(1, Ordering::SeqCst);
    path.with_file_name(format!(".{name}.{}.{number}.tmp", std::process::id()))
}

pub fn write_private(path: &Path, content: &[u8], overwrite: bool) -> Result<()> {
    if path.is_dir() {
        return Err(AppError::Usage(format!("\"{}\" is a directory: choose a file path", path.display())));
    }
    if !overwrite && path.exists() {
        return Err(AppError::Conflict(format!(
            "\"{}\" already exists and was not changed: choose another name or allow overwriting",
            path.display()
        )));
    }
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        create_private_dir(parent)?;
    }
    let temporary = temporary_sibling(path);
    let written = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        restrict(&temporary, false)?;
        file.write_all(content)?;
        file.sync_all()?;
        Ok(())
    })();
    let finished = written.and_then(|()| fs::rename(&temporary, path).map_err(AppError::from));
    if let Err(error) = finished {
        let _ = fs::remove_file(&temporary);
        return Err(AppError::Io(format!("cannot write \"{}\": {error}", path.display())));
    }
    Ok(())
}

pub fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let size =
        fs::metadata(path).map_err(|error| AppError::Io(format!("cannot read \"{}\": {error}", path.display())))?.len();
    if size > limit {
        return Err(AppError::Usage(format!("\"{}\" is larger than {limit} bytes and was not read", path.display())));
    }
    fs::read(path).map_err(|error| AppError::Io(format!("cannot read \"{}\": {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::TempDir;

    #[test]
    fn files_are_written_whole_private_and_never_overwritten_silently() {
        let directory = TempDir::new("files");
        let path = directory.path().join("nested").join("export.json");
        write_private(&path, b"first", false).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first");
        let refused = write_private(&path, b"second", false).unwrap_err();
        assert!(refused.to_string().contains("already exists"), "{refused}");
        assert_eq!(fs::read(&path).unwrap(), b"first");
        write_private(&path, b"second", true).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        let names: Vec<String> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["export.json"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
            assert_eq!(fs::metadata(path.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        }
        assert!(write_private(directory.path(), b"x", true).is_err());
    }

    #[test]
    fn oversized_and_missing_files_are_not_read() {
        let directory = TempDir::new("files-limit");
        let path = directory.path().join("big.txt");
        fs::write(&path, vec![b'a'; 100]).unwrap();
        assert_eq!(read_limited(&path, 100).unwrap().len(), 100);
        assert!(read_limited(&path, 99).unwrap_err().to_string().contains("larger than 99 bytes"));
        assert!(read_limited(&directory.path().join("missing"), 10).is_err());
    }
}
