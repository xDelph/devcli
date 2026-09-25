/// File discovery + classification.
///
/// Walks the root directory (non-recursively, one level of named sub-dirs)
/// looking for files that match `.env` naming conventions and classifying them
/// by stage, context, and naming style.
use std::path::{Path, PathBuf};

use crate::Result;

/// How was the context specified?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamingConvention {
    /// Context embedded in the file name: `.env.docker`, `.env.dev.docker`
    Suffix,
    /// Context expressed as a directory: `docker/.env`, `docker/.env.dev`
    Directory,
    /// No explicit context in the name
    None,
}

/// A raw detected env file before resolver layer assignment.
#[derive(Debug, Clone)]
pub struct DetectedFile {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Path relative to the root directory (e.g. `"docker/.env.dev"`).
    pub relative_path: String,
    /// The detected stage, if any.
    pub stage: Option<String>,
    /// The detected context, if any (e.g. `"docker"`, `"k8s"`).
    pub context: Option<String>,
    /// Whether this is a `.local` file.
    pub is_local: bool,
    /// How the context was expressed.
    pub convention: NamingConvention,
}

/// Discover all env files under `root`.
///
/// Searches `root/` for `.env*` files, and each immediate sub-directory
/// for `.env*` files (supporting directory-style context convention).
pub fn discover(root: &Path) -> Result<Vec<DetectedFile>> {
    let mut results = Vec::new();

    if !root.exists() || !root.is_dir() {
        return Err(crate::Error::InvalidRoot(root.to_path_buf()));
    }

    // Scan root itself
    results.extend(scan_dir(root, root, None)?);

    // Scan immediate sub-directories (for `docker/.env`, `k8s/.env`, etc.)
    for entry in std::fs::read_dir(root).map_err(|e| crate::Error::Io {
        path: root.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| crate::Error::Io {
            path: root.to_path_buf(),
            source: e,
        })?;
        let sub = entry.path();
        if sub.is_dir() {
            let dir_name = sub
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());
            results.extend(scan_dir(&sub, root, dir_name.as_deref())?);
        }
    }

    Ok(results)
}

/// Scan one directory for `.env*` files.
///
/// `dir_context` is set when scanning a sub-directory (directory-style convention).
fn scan_dir(dir: &Path, root: &Path, dir_context: Option<&str>) -> Result<Vec<DetectedFile>> {
    let mut results = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(results),
    };

    for entry in entries {
        let entry = entry.map_err(|e| crate::Error::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(f) => f,
            None => continue,
        };
        if !is_env_filename(filename) {
            continue;
        }
        let relative_path = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| filename.to_string());

        let mut detected = classify(filename, dir_context);
        detected.path = path;
        detected.relative_path = relative_path;

        results.push(detected);
    }

    Ok(results)
}

/// Returns true if `filename` matches a `.env` naming pattern.
///
/// Supports standard (`.env`, `.env.dev`) and reverse (`.local.env`, `.dev.env`) styles.
fn is_env_filename(filename: &str) -> bool {
    filename == ".env"
        || filename.starts_with(".env.")
        || (filename.starts_with('.') && filename.ends_with(".env"))
}

/// Classify a filename into stage / context / is_local.
///
/// `dir_context` is Some("docker") when file lives in `docker/` sub-dir.
fn classify(filename: &str, dir_context: Option<&str>) -> DetectedFile {
    let mut stage: Option<String> = None;
    let mut context: Option<String> = dir_context.map(|s| s.to_string());
    let mut is_local = false;
    let mut convention = if dir_context.is_some() {
        NamingConvention::Directory
    } else {
        NamingConvention::None
    };

    // Reverse pattern: `.local.env`, `.dev.env`, `.dev.local.env`
    if filename.ends_with(".env") && filename != ".env" && !filename.starts_with(".env.") {
        if let Some(inner) = filename
            .strip_prefix('.')
            .and_then(|s| s.strip_suffix(".env"))
        {
            let parts: Vec<&str> = inner.split('.').collect();
            let non_local: Vec<&str> = parts.iter().copied().filter(|&p| p != "local").collect();

            if parts.last() == Some(&"local") {
                is_local = true;
            }

            match non_local.len() {
                0 => {}
                1 => {
                    let token = non_local[0];
                    if is_known_context(token) {
                        context = Some(token.to_string());
                    } else {
                        stage = Some(token.to_string());
                    }
                }
                2 => {
                    stage = Some(non_local[0].to_string());
                    context = Some(non_local[1].to_string());
                }
                _ => {
                    stage = Some(non_local[0].to_string());
                    if let Some(last) = non_local.last() {
                        context = Some(last.to_string());
                    }
                }
            }

            return DetectedFile {
                path: PathBuf::new(),
                relative_path: String::new(),
                stage,
                context,
                is_local,
                convention: NamingConvention::Suffix,
            };
        }
    }

    // Suffix after `.env`
    if let Some(suffix) = filename.strip_prefix(".env.") {
        // suffix is "dev", "dev.local", "dev.docker", "local", "docker", etc.
        let parts: Vec<&str> = suffix.split('.').collect();

        // If the last token is "local", this is a `.local` file
        if parts.last() == Some(&"local") {
            is_local = true;
        }

        // Classify the remaining tokens
        let non_local: Vec<&str> = parts.iter().copied().filter(|&p| p != "local").collect();

        match non_local.len() {
            0 => {
                // `.env.local`
                // is_local already set
            }
            1 => {
                let token = non_local[0];
                if is_known_context(token) {
                    // `.env.docker` → context only
                    context = Some(token.to_string());
                    convention = NamingConvention::Suffix;
                } else {
                    // `.env.dev` → stage only
                    stage = Some(token.to_string());
                }
            }
            2 => {
                // `.env.dev.docker` → stage + context
                let a = non_local[0];
                let b = non_local[1];
                if is_known_context(b) {
                    stage = Some(a.to_string());
                    context = Some(b.to_string());
                    convention = NamingConvention::Suffix;
                } else if is_known_context(a) {
                    // unusual: `.env.docker.dev` — treat first as context
                    context = Some(a.to_string());
                    stage = Some(b.to_string());
                    convention = NamingConvention::Suffix;
                } else {
                    // Both unknown → first=stage, second=context (best-effort)
                    stage = Some(a.to_string());
                    context = Some(b.to_string());
                }
            }
            _ => {
                // More tokens: first=stage, last non-local=context
                stage = Some(non_local[0].to_string());
                if let Some(last) = non_local.last() {
                    context = Some(last.to_string());
                    convention = NamingConvention::Suffix;
                }
            }
        }
    }
    // else: bare `.env` in root or in a sub-dir (dir_context already set)

    DetectedFile {
        path: PathBuf::new(),         // filled in by caller
        relative_path: String::new(), // filled in by caller
        stage,
        context,
        is_local,
        convention,
    }
}

/// Known runtime context names (used to distinguish stage tokens from context tokens).
fn is_known_context(s: &str) -> bool {
    matches!(
        s,
        "docker" | "k8s" | "kubernetes" | "orbstack" | "ci" | "compose"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cls(filename: &str) -> DetectedFile {
        classify(filename, None)
    }

    fn cls_dir(filename: &str, dir: &str) -> DetectedFile {
        classify(filename, Some(dir))
    }

    #[test]
    fn bare_env() {
        let f = cls(".env");
        assert_eq!(f.stage, None);
        assert_eq!(f.context, None);
        assert!(!f.is_local);
        assert_eq!(f.convention, NamingConvention::None);
    }

    #[test]
    fn env_stage() {
        let f = cls(".env.dev");
        assert_eq!(f.stage, Some("dev".to_string()));
        assert_eq!(f.context, None);
        assert!(!f.is_local);
    }

    #[test]
    fn env_local() {
        let f = cls(".env.local");
        assert_eq!(f.stage, None);
        assert_eq!(f.context, None);
        assert!(f.is_local);
    }

    #[test]
    fn env_stage_local() {
        let f = cls(".env.dev.local");
        assert_eq!(f.stage, Some("dev".to_string()));
        assert!(f.is_local);
    }

    #[test]
    fn env_context_suffix() {
        let f = cls(".env.docker");
        assert_eq!(f.context, Some("docker".to_string()));
        assert_eq!(f.stage, None);
        assert_eq!(f.convention, NamingConvention::Suffix);
    }

    #[test]
    fn env_stage_context_suffix() {
        let f = cls(".env.dev.docker");
        assert_eq!(f.stage, Some("dev".to_string()));
        assert_eq!(f.context, Some("docker".to_string()));
        assert_eq!(f.convention, NamingConvention::Suffix);
    }

    #[test]
    fn env_dir_style() {
        let f = cls_dir(".env", "docker");
        assert_eq!(f.context, Some("docker".to_string()));
        assert_eq!(f.convention, NamingConvention::Directory);
    }

    #[test]
    fn env_stage_dir_style() {
        let f = cls_dir(".env.dev", "docker");
        assert_eq!(f.stage, Some("dev".to_string()));
        assert_eq!(f.context, Some("docker".to_string()));
        assert_eq!(f.convention, NamingConvention::Directory);
    }

    #[test]
    fn env_k8s_suffix() {
        let f = cls(".env.k8s");
        assert_eq!(f.context, Some("k8s".to_string()));
        assert_eq!(f.convention, NamingConvention::Suffix);
    }

    #[test]
    fn env_ci_stage() {
        let f = cls(".env.ci");
        assert_eq!(f.context, Some("ci".to_string()));
    }

    #[test]
    fn reverse_local_env() {
        let f = cls(".local.env");
        assert!(f.is_local);
        assert_eq!(f.stage, None);
    }

    #[test]
    fn reverse_stage_env() {
        let f = cls(".dev.env");
        assert_eq!(f.stage, Some("dev".to_string()));
        assert!(!f.is_local);
    }

    #[test]
    fn reverse_stage_local_env() {
        let f = cls(".dev.local.env");
        assert_eq!(f.stage, Some("dev".to_string()));
        assert!(f.is_local);
    }
}
