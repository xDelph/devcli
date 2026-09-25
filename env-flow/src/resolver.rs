/// Layer chain algorithm.
///
/// Given (root, stage, context) → ordered `Vec<ResolvedLayer>` from lowest to
/// highest priority.  Layers that don't exist on disk still appear in the list
/// (`exists: false`) for transparency.
use std::path::Path;

use crate::{
    types::{LayerType, ResolvedLayer, RuntimeContext, Stage},
    Result,
};

/// Build the ordered layer chain for the given combination.
pub fn resolve(
    root: &Path,
    stage: Option<&Stage>,
    context: &RuntimeContext,
) -> Result<Vec<ResolvedLayer>> {
    let skip_local = context.skip_local_files();
    // For Local context there is no context-specific file to load
    let skip_context = matches!(context, RuntimeContext::Local);
    let ctx_str = context.as_str();

    let mut layers = Vec::new();

    // ── Layer 1: base .env ───────────────────────────────────────────────────
    layers.push(make_layer(root, ".env", LayerType::Base));

    // ── Layer 2: context base (.env.{ctx} OR {ctx}/.env) ────────────────────
    if !skip_context {
        let layer = probe_either(
            root,
            &format!(".env.{ctx_str}"),
            &format!("{ctx_str}/.env"),
            LayerType::ContextBase,
        );
        layers.push(layer);
    }

    if let Some(s) = stage {
        let stage_str = s.as_str();

        // ── Layer 3: stage base (.env.{stage}) ──────────────────────────────
        layers.push(make_layer(
            root,
            &format!(".env.{stage_str}"),
            LayerType::StageBase,
        ));

        // ── Layer 4: stage + context (.env.{stage}.{ctx} OR {ctx}/.env.{stage})
        if !skip_context {
            let layer = probe_either(
                root,
                &format!(".env.{stage_str}.{ctx_str}"),
                &format!("{ctx_str}/.env.{stage_str}"),
                LayerType::StageContext,
            );
            layers.push(layer);
        }
    }

    // ── Layer 5: .env.local / .local.env (skip in containers/CI) ─────────────
    if !skip_local {
        layers.push(probe_either(
            root,
            ".env.local",
            ".local.env",
            LayerType::LocalOverride,
        ));

        // ── Layer 6: .env.{stage}.local / .{stage}.local.env ────────────────
        if let Some(s) = stage {
            let stage_str = s.as_str();
            layers.push(probe_either(
                root,
                &format!(".env.{stage_str}.local"),
                &format!(".{stage_str}.local.env"),
                LayerType::StageLocalOverride,
            ));
        }
    }

    Ok(layers)
}

/// Build a `ResolvedLayer` for a single relative path under `root`.
fn make_layer(root: &Path, rel: &str, layer_type: LayerType) -> ResolvedLayer {
    let path = root.join(rel);
    let exists = path.exists();
    ResolvedLayer {
        path,
        relative_path: rel.to_string(),
        exists,
        layer_type,
    }
}

/// Prefer suffix-style if it exists on disk; fall back to directory-style;
/// if neither exists, return the suffix-style layer with `exists: false`.
fn probe_either(
    root: &Path,
    suffix_rel: &str,
    dir_rel: &str,
    layer_type: LayerType,
) -> ResolvedLayer {
    let suffix_path = root.join(suffix_rel);
    if suffix_path.exists() {
        return ResolvedLayer {
            path: suffix_path,
            relative_path: suffix_rel.to_string(),
            exists: true,
            layer_type,
        };
    }
    let dir_path = root.join(dir_rel);
    if dir_path.exists() {
        return ResolvedLayer {
            path: dir_path,
            relative_path: dir_rel.to_string(),
            exists: true,
            layer_type,
        };
    }
    // Neither exists → return suffix-style as placeholder
    ResolvedLayer {
        path: suffix_path,
        relative_path: suffix_rel.to_string(),
        exists: false,
        layer_type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        TempDir::new().unwrap()
    }

    fn touch(dir: &Path, rel: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"").unwrap();
    }

    #[test]
    fn local_no_stage() {
        let dir = tmp();
        let layers = resolve(dir.path(), None, &RuntimeContext::Local).unwrap();
        // Local + no stage → base + local-override only
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].layer_type, LayerType::Base);
        assert_eq!(layers[1].layer_type, LayerType::LocalOverride);
    }

    #[test]
    fn local_with_stage() {
        let dir = tmp();
        let layers = resolve(dir.path(), Some(&Stage::Dev), &RuntimeContext::Local).unwrap();
        assert_eq!(layers.len(), 4);
        assert_eq!(layers[0].layer_type, LayerType::Base);
        assert_eq!(layers[1].layer_type, LayerType::StageBase);
        assert_eq!(layers[2].layer_type, LayerType::LocalOverride);
        assert_eq!(layers[3].layer_type, LayerType::StageLocalOverride);
    }

    #[test]
    fn docker_no_stage() {
        let dir = tmp();
        let layers = resolve(dir.path(), None, &RuntimeContext::Docker).unwrap();
        // Docker + no stage → base + context-base (no local files)
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].layer_type, LayerType::Base);
        assert_eq!(layers[1].layer_type, LayerType::ContextBase);
        for l in &layers {
            assert!(!matches!(
                l.layer_type,
                LayerType::LocalOverride | LayerType::StageLocalOverride
            ));
        }
    }

    #[test]
    fn docker_with_stage() {
        let dir = tmp();
        let layers = resolve(dir.path(), Some(&Stage::Prod), &RuntimeContext::Docker).unwrap();
        assert_eq!(layers.len(), 4);
        assert_eq!(layers[0].layer_type, LayerType::Base);
        assert_eq!(layers[1].layer_type, LayerType::ContextBase);
        assert_eq!(layers[2].layer_type, LayerType::StageBase);
        assert_eq!(layers[3].layer_type, LayerType::StageContext);
    }

    #[test]
    fn suffix_style_preferred_over_dir() {
        let dir = tmp();
        // Both exist
        touch(dir.path(), ".env.docker");
        touch(dir.path(), "docker/.env");

        let layers = resolve(dir.path(), None, &RuntimeContext::Docker).unwrap();
        let ctx = layers
            .iter()
            .find(|l| l.layer_type == LayerType::ContextBase)
            .unwrap();
        assert_eq!(ctx.relative_path, ".env.docker");
        assert!(ctx.exists);
    }

    #[test]
    fn dir_style_fallback() {
        let dir = tmp();
        // Only dir-style exists
        touch(dir.path(), "docker/.env");

        let layers = resolve(dir.path(), None, &RuntimeContext::Docker).unwrap();
        let ctx = layers
            .iter()
            .find(|l| l.layer_type == LayerType::ContextBase)
            .unwrap();
        assert_eq!(ctx.relative_path, "docker/.env");
        assert!(ctx.exists);
    }

    #[test]
    fn nonexistent_files_appear_with_exists_false() {
        let dir = tmp();
        // Only create base .env
        touch(dir.path(), ".env");

        let layers = resolve(dir.path(), Some(&Stage::Dev), &RuntimeContext::Local).unwrap();
        let stage_base = layers
            .iter()
            .find(|l| l.layer_type == LayerType::StageBase)
            .unwrap();
        assert!(!stage_base.exists);
        assert_eq!(stage_base.relative_path, ".env.dev");
    }

    #[test]
    fn ci_skips_local_files() {
        let dir = tmp();
        touch(dir.path(), ".env.local");
        touch(dir.path(), ".env.dev.local");

        let layers = resolve(dir.path(), Some(&Stage::Dev), &RuntimeContext::CI).unwrap();
        assert!(!layers.iter().any(|l| matches!(
            l.layer_type,
            LayerType::LocalOverride | LayerType::StageLocalOverride
        )));
    }

    #[test]
    fn k8s_resolves_correct_path() {
        let dir = tmp();
        touch(dir.path(), "k8s/.env");

        let layers = resolve(dir.path(), None, &RuntimeContext::Kubernetes).unwrap();
        let ctx = layers
            .iter()
            .find(|l| l.layer_type == LayerType::ContextBase)
            .unwrap();
        assert_eq!(ctx.relative_path, "k8s/.env");
        assert!(ctx.exists);
    }

    #[test]
    fn reverse_local_env_style() {
        let dir = tmp();
        touch(dir.path(), ".local.env");

        let layers = resolve(dir.path(), None, &RuntimeContext::Local).unwrap();
        let local = layers
            .iter()
            .find(|l| l.layer_type == LayerType::LocalOverride)
            .unwrap();
        assert_eq!(local.relative_path, ".local.env");
        assert!(local.exists);
    }
}
