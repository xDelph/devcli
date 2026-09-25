/// Load and merge resolved layers into a single `EnvVars` map.
///
/// Higher layers win: later values overwrite earlier ones, but the old `source_file`
/// is recorded in `EnvEntry::overridden_by` for debugging.
use std::fs;
use std::path::Path;

use indexmap::IndexMap;

use crate::{
    parser,
    types::{EnvEntry, EnvVars, ParsedEntry, ResolvedLayer},
    Error, Result,
};

/// Load all layers (in order) into a merged `EnvVars`.
///
/// `root` is only used for error context; file paths come from the layers.
pub fn load(layers: &[ResolvedLayer], strict: bool) -> Result<EnvVars> {
    let mut map: IndexMap<String, EnvEntry> = IndexMap::new();

    for layer in layers {
        if !layer.exists {
            tracing::debug!(
                path = %layer.relative_path,
                "layer does not exist, skipping"
            );
            continue;
        }

        let entries = load_file(&layer.path, strict)?;
        tracing::debug!(
            path = %layer.relative_path,
            count = entries.len(),
            "loaded layer"
        );

        for entry in entries {
            let parsed = entry;
            if let Some(existing) = map.get_mut(&parsed.key) {
                // Higher layer overrides
                existing.overridden_by.push(existing.source_file.clone());
                existing.source_file = layer.path.clone();
                existing.source_line = parsed.line_number;
                // raw_value here is already unquoted/escaped by the parser;
                // interpolation will happen in a later pass
                existing.value = parsed.raw_value;
                existing.interpolate = parsed.interpolate;
            } else {
                map.insert(
                    parsed.key.clone(),
                    EnvEntry {
                        value: parsed.raw_value,
                        source_file: layer.path.clone(),
                        source_line: parsed.line_number,
                        overridden_by: Vec::new(),
                        interpolate: parsed.interpolate,
                    },
                );
            }
        }
    }

    Ok(EnvVars(map))
}

/// Load a single file and return parsed entries.
pub(crate) fn load_file(path: &Path, strict: bool) -> Result<Vec<ParsedEntry>> {
    let content = fs::read_to_string(path).map_err(|e| Error::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let parsed = parser::parse(&content, path, strict)?;
    Ok(parsed.entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LayerType;
    use std::fs;
    use tempfile::TempDir;

    fn touch(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    fn layer(dir: &Path, name: &str, layer_type: LayerType) -> ResolvedLayer {
        let path = dir.join(name);
        ResolvedLayer {
            exists: path.exists(),
            path,
            relative_path: name.to_string(),
            layer_type,
        }
    }

    #[test]
    fn basic_merge() {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), ".env", "PORT=3000\nDB=localhost\n");
        touch(dir.path(), ".env.dev", "DB=dev-db\n");

        let layers = vec![
            layer(dir.path(), ".env", LayerType::Base),
            layer(dir.path(), ".env.dev", LayerType::StageBase),
        ];
        let vars = load(&layers, false).unwrap();
        assert_eq!(vars.get("PORT"), Some("3000"));
        assert_eq!(vars.get("DB"), Some("dev-db")); // overridden by .env.dev
    }

    #[test]
    fn source_tracking() {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), ".env", "KEY=base\n");
        touch(dir.path(), ".env.dev", "KEY=dev\n");

        let layers = vec![
            layer(dir.path(), ".env", LayerType::Base),
            layer(dir.path(), ".env.dev", LayerType::StageBase),
        ];
        let vars = load(&layers, false).unwrap();
        let entry = vars.0.get("KEY").unwrap();
        assert_eq!(entry.value, "dev");
        // The base file should be in overridden_by
        assert_eq!(entry.overridden_by.len(), 1);
        assert!(entry.source_file.to_string_lossy().contains(".env.dev"));
    }

    #[test]
    fn nonexistent_layers_skipped() {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), ".env", "KEY=val\n");

        let layers = vec![
            layer(dir.path(), ".env", LayerType::Base),
            // .env.dev does not exist
            layer(dir.path(), ".env.dev", LayerType::StageBase),
        ];
        let vars = load(&layers, false).unwrap();
        assert_eq!(vars.get("KEY"), Some("val"));
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn empty_values_preserved() {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), ".env", "KEY=\nANOTHER=value\n");

        let layers = vec![layer(dir.path(), ".env", LayerType::Base)];
        let vars = load(&layers, false).unwrap();
        // Empty values are kept as-is (not replaced with "XXX" like the old code)
        assert_eq!(vars.get("KEY"), Some(""));
        assert_eq!(vars.get("ANOTHER"), Some("value"));
    }

    #[test]
    fn insertion_order_preserved() {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), ".env", "Z=1\nA=2\nM=3\n");

        let layers = vec![layer(dir.path(), ".env", LayerType::Base)];
        let vars = load(&layers, false).unwrap();
        let keys: Vec<&str> = vars.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["Z", "A", "M"]);
    }
}
