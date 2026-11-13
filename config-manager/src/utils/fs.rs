//! File-system helpers.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Replace the file at `path` with `contents` in a single step.
///
/// Writes a sibling temp file, flushes it to disk and renames it over `path`.
/// A plain `fs::write` truncates first, so a crash, a full disk or a concurrent
/// reader could observe an empty or half-written configuration - after which
/// every command failed to parse it. The permissions of an existing file are
/// kept (a config deliberately made 0600 stays 0600).
///
/// A symlink at `path` is followed: the file it points to is replaced, next to
/// itself, and the link stays a link. Renaming over the link itself replaced a
/// `config.json` kept in a dotfiles repository with a plain copy, which then
/// silently diverged from the repository.
pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let target = link_target(path);
    let path = target.as_path();
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config".to_string());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.{nanos}.tmp", std::process::id()));

    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        if let Ok(existing) = std::fs::metadata(path) {
            file.set_permissions(existing.permissions())?;
        }
        file.write_all(contents)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// The file a write to `path` should replace: `path` itself, or the file a
/// symlink at `path` (or a chain of them) points to, even when it does not
/// exist yet.
fn link_target(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    // Bounded, so a link cycle cannot loop forever (the OS gives up at ~40).
    for _ in 0..40 {
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => match std::fs::read_link(&current) {
                Ok(target) if target.is_relative() => {
                    current = current
                        .parent()
                        .map(|dir| dir.join(&target))
                        .unwrap_or(target);
                }
                Ok(target) => current = target,
                Err(_) => return current,
            },
            _ => return current,
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_content_and_leaves_no_temp_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    #[cfg(unix)]
    fn keeps_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        write_atomic(&path, b"new").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    #[cfg(unix)]
    fn writes_through_a_symlink_and_keeps_it() {
        let dir = tempfile::TempDir::new().unwrap();
        let dotfiles = dir.path().join("dotfiles");
        std::fs::create_dir(&dotfiles).unwrap();
        let real = dotfiles.join("config.json");
        std::fs::write(&real, b"old").unwrap();
        let link = dir.path().join("config.json");
        std::os::unix::fs::symlink("dotfiles/config.json", &link).unwrap();

        write_atomic(&link, b"new").unwrap();

        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read(&real).unwrap(), b"new");
        // The temp file was created next to the target and is gone.
        assert_eq!(std::fs::read_dir(&dotfiles).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    #[cfg(unix)]
    fn creates_the_target_of_a_dangling_symlink() {
        let dir = tempfile::TempDir::new().unwrap();
        let real = dir.path().join("real.json");
        let link = dir.path().join("config.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        write_atomic(&link, b"new").unwrap();

        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read(&real).unwrap(), b"new");
    }
}
