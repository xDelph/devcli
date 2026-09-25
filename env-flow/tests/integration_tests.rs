/// Integration tests using the on-disk fixture directories.
use env_flow::{EnvFlow, LayerType, RuntimeContext, Stage};
use std::path::Path;

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

fn fix(name: &str) -> std::path::PathBuf {
    fixtures().join(name)
}

// ── minimal ──────────────────────────────────────────────────────────────────

#[test]
fn minimal_base() {
    let vars = EnvFlow::from_dir(fix("minimal")).load().unwrap();
    assert_eq!(vars.get("PORT"), Some("3000"));
    assert_eq!(vars.get("DB_HOST"), Some("localhost"));
}

// ── local-overrides ──────────────────────────────────────────────────────────

#[test]
fn local_overrides_loaded_in_local_context() {
    let vars = EnvFlow::from_dir(fix("local-overrides"))
        .context(RuntimeContext::Local)
        .load()
        .unwrap();
    assert_eq!(vars.get("DEBUG"), Some("true")); // from .env.local
    assert_eq!(vars.get("SECRET_KEY"), Some("localdev"));
    assert_eq!(vars.get("PORT"), Some("3000")); // inherited from .env
}

#[test]
fn local_overrides_skipped_in_docker() {
    let vars = EnvFlow::from_dir(fix("local-overrides"))
        .context(RuntimeContext::Docker)
        .load()
        .unwrap();
    assert_eq!(vars.get("DEBUG"), Some("false")); // .env.local NOT loaded
}

// ── staged ───────────────────────────────────────────────────────────────────

#[test]
fn staged_dev() {
    let vars = EnvFlow::from_dir(fix("staged"))
        .stage(Stage::Dev)
        .load()
        .unwrap();
    assert_eq!(vars.get("DB_HOST"), Some("dev-db"));
    assert_eq!(vars.get("APP_NAME"), Some("myapp")); // from base
}

#[test]
fn staged_prod() {
    let vars = EnvFlow::from_dir(fix("staged"))
        .stage(Stage::Prod)
        .load()
        .unwrap();
    assert_eq!(vars.get("DB_HOST"), Some("prod-db"));
    assert_eq!(vars.get("DEBUG"), Some("false"));
}

#[test]
fn staged_no_stage_uses_base() {
    let vars = EnvFlow::from_dir(fix("staged")).load().unwrap();
    assert_eq!(vars.get("DB_HOST"), Some("localhost")); // only base
}

// ── staged-with-local ────────────────────────────────────────────────────────

#[test]
fn staged_with_local_full_chain() {
    let vars = EnvFlow::from_dir(fix("staged-with-local"))
        .stage(Stage::Dev)
        .context(RuntimeContext::Local)
        .load()
        .unwrap();
    assert_eq!(vars.get("APP_NAME"), Some("myapp"));
    assert_eq!(vars.get("DB_HOST"), Some("dev-db"));
    assert_eq!(vars.get("SECRET_KEY"), Some("my-local-secret"));
    assert_eq!(vars.get("DB_PASSWORD"), Some("local-dev-password"));
}

#[test]
fn staged_with_local_docker_skips_local() {
    let vars = EnvFlow::from_dir(fix("staged-with-local"))
        .stage(Stage::Dev)
        .context(RuntimeContext::Docker)
        .load()
        .unwrap();
    assert_eq!(vars.get("SECRET_KEY"), None); // .env.local skipped
    assert_eq!(vars.get("DB_PASSWORD"), None); // .env.dev.local skipped
}

// ── docker-dir-style ─────────────────────────────────────────────────────────

#[test]
fn docker_dir_style_base() {
    let vars = EnvFlow::from_dir(fix("docker-dir-style"))
        .context(RuntimeContext::Docker)
        .load()
        .unwrap();
    assert_eq!(vars.get("PORT"), Some("80")); // docker/.env overrides .env
}

#[test]
fn docker_dir_style_with_stage() {
    let vars = EnvFlow::from_dir(fix("docker-dir-style"))
        .stage(Stage::Dev)
        .context(RuntimeContext::Docker)
        .load()
        .unwrap();
    assert_eq!(vars.get("PORT"), Some("8080")); // docker/.env.dev wins
}

// ── docker-suffix-style ──────────────────────────────────────────────────────

#[test]
fn docker_suffix_style_base() {
    let vars = EnvFlow::from_dir(fix("docker-suffix-style"))
        .context(RuntimeContext::Docker)
        .load()
        .unwrap();
    assert_eq!(vars.get("PORT"), Some("80")); // .env.docker
}

#[test]
fn docker_suffix_style_with_stage() {
    let vars = EnvFlow::from_dir(fix("docker-suffix-style"))
        .stage(Stage::Dev)
        .context(RuntimeContext::Docker)
        .load()
        .unwrap();
    assert_eq!(vars.get("PORT"), Some("8080")); // .env.dev.docker
    assert_eq!(vars.get("DEBUG"), Some("false"));
}

// ── kubernetes ───────────────────────────────────────────────────────────────

#[test]
fn kubernetes_prod_full() {
    let vars = EnvFlow::from_dir(fix("kubernetes"))
        .stage(Stage::Prod)
        .context(RuntimeContext::Kubernetes)
        .load()
        .unwrap();
    assert_eq!(vars.get("APP_NAME"), Some("myapp"));
    assert_eq!(vars.get("REPLICAS"), Some("3")); // .env.prod
    assert_eq!(vars.get("K8S_NAMESPACE"), Some("production")); // k8s/.env
    assert_eq!(vars.get("INGRESS_HOST"), Some("api.example.com")); // k8s/.env.prod
}

// ── ci ───────────────────────────────────────────────────────────────────────

#[test]
fn ci_loads_ci_file() {
    let vars = EnvFlow::from_dir(fix("ci"))
        .context(RuntimeContext::CI)
        .load()
        .unwrap();
    assert_eq!(vars.get("APP_NAME"), Some("myapp"));
    assert_eq!(vars.get("CI_TIMEOUT"), Some("60"));
}

// ── full-stack ───────────────────────────────────────────────────────────────

#[test]
fn full_stack_local_dev() {
    let vars = EnvFlow::from_dir(fix("full-stack"))
        .stage(Stage::Dev)
        .context(RuntimeContext::Local)
        .load()
        .unwrap();
    assert_eq!(vars.get("PORT"), Some("3001")); // .env.dev
    assert_eq!(vars.get("SECRET"), Some("local-secret")); // .env.local
    assert_eq!(vars.get("DB_PASSWORD"), Some("local-dev-pass")); // .env.dev.local
}

#[test]
fn full_stack_docker_prod() {
    let vars = EnvFlow::from_dir(fix("full-stack"))
        .stage(Stage::Prod)
        .context(RuntimeContext::Docker)
        .load()
        .unwrap();
    assert_eq!(vars.get("PORT"), Some("443")); // docker/.env.prod
    assert_eq!(vars.get("SECRET"), None); // .env.local skipped
}

// ── edge-cases ───────────────────────────────────────────────────────────────

#[test]
fn edge_empty_dir() {
    let vars = EnvFlow::from_dir(fix("edge-cases/empty-dir"))
        .load()
        .unwrap();
    assert!(vars.is_empty());
}

#[test]
fn edge_empty_values() {
    let vars = EnvFlow::from_dir(fix("edge-cases/empty-values"))
        .load()
        .unwrap();
    assert_eq!(vars.get("KEY"), Some(""));
    assert_eq!(vars.get("ANOTHER"), Some("value"));
}

#[test]
fn edge_quoted_values() {
    let vars = EnvFlow::from_dir(fix("edge-cases/quoted-values"))
        .load()
        .unwrap();
    assert_eq!(vars.get("DOUBLE"), Some("hello world"));
    assert_eq!(vars.get("SINGLE"), Some("hello world"));
}

#[test]
fn edge_equals_in_value() {
    let vars = EnvFlow::from_dir(fix("edge-cases/equals-in-value"))
        .load()
        .unwrap();
    assert_eq!(
        vars.get("DATABASE_URL"),
        Some("postgres://user:pass@host/db?ssl=true")
    );
}

#[test]
fn edge_export_prefix() {
    let vars = EnvFlow::from_dir(fix("edge-cases/export-prefix"))
        .load()
        .unwrap();
    assert_eq!(vars.get("KEY"), Some("value"));
}

#[test]
fn edge_inline_comments() {
    let vars = EnvFlow::from_dir(fix("edge-cases/inline-comments"))
        .load()
        .unwrap();
    assert_eq!(vars.get("KEY"), Some("value"));
    assert_eq!(vars.get("URL"), Some("https://x.com#fragment"));
}

#[test]
fn edge_multiline() {
    let vars = EnvFlow::from_dir(fix("edge-cases/multiline"))
        .load()
        .unwrap();
    let cert = vars.get("CERT").unwrap();
    assert!(cert.contains("BEGIN CERT"));
    assert!(cert.contains('\n'));
}

#[test]
fn edge_interpolation() {
    let vars = EnvFlow::from_dir(fix("edge-cases/interpolation"))
        .load()
        .unwrap();
    assert_eq!(vars.get("GREETING"), Some("hello_world"));
}

#[test]
fn edge_circular_error() {
    let result = EnvFlow::from_dir(fix("edge-cases/circular")).load();
    assert!(result.is_err());
}

#[test]
fn edge_missing_base() {
    // .env doesn't exist — not an error, just nothing from base
    let vars = EnvFlow::from_dir(fix("edge-cases/missing-base"))
        .stage(Stage::Dev)
        .load()
        .unwrap();
    assert_eq!(vars.get("ONLY_DEV"), Some("true"));
}

#[test]
fn edge_unicode() {
    let vars = EnvFlow::from_dir(fix("edge-cases/unicode")).load().unwrap();
    assert_eq!(vars.get("GREETING"), Some("héllo wörld"));
    assert_eq!(vars.get("JAPANESE"), Some("日本語"));
}

// ── layer plan ───────────────────────────────────────────────────────────────

#[test]
fn layers_plan_transparency() {
    let plan = EnvFlow::from_dir(fix("staged"))
        .stage(Stage::Dev)
        .context(RuntimeContext::Local)
        .layers()
        .unwrap();

    // 4 layers: base, stage, local-override, stage-local-override
    assert_eq!(plan.len(), 4);
    let existing: Vec<_> = plan.iter().filter(|l| l.exists).collect();
    // .env and .env.dev exist; .env.local and .env.dev.local do not
    assert_eq!(existing.len(), 2);
    assert!(existing.iter().any(|l| l.layer_type == LayerType::Base));
    assert!(existing
        .iter()
        .any(|l| l.layer_type == LayerType::StageBase));
}

// ── no-cascade fixture ───────────────────────────────────────────────────────
//
// Fixture layout (deliberately non-overlapping keys per layer):
//   .env            → PORT=3000, APP_NAME=myapp
//   .env.dev        → DB=dev-db, DEBUG=true
//   .env.dev.local  → SECRET=local-secret, DEV_NOTE=override

#[test]
fn no_cascade_fixture_cascade_merges_all_layers() {
    // Default (cascade): all three layers merged, 4 unique keys total + DEV_NOTE
    let vars = EnvFlow::from_dir(fix("no-cascade"))
        .stage(Stage::Dev)
        .context(RuntimeContext::Local)
        .load()
        .unwrap();

    assert_eq!(vars.get("PORT"), Some("3000"));
    assert_eq!(vars.get("APP_NAME"), Some("myapp"));
    assert_eq!(vars.get("DB"), Some("dev-db"));
    assert_eq!(vars.get("DEBUG"), Some("true"));
    assert_eq!(vars.get("SECRET"), Some("local-secret"));
    assert_eq!(vars.get("DEV_NOTE"), Some("override"));
    assert_eq!(vars.len(), 6);
}

#[test]
fn no_cascade_fixture_no_cascade_picks_highest_existing() {
    // no_cascade + dev + local → highest existing = .env.dev.local
    let vars = EnvFlow::from_dir(fix("no-cascade"))
        .stage(Stage::Dev)
        .context(RuntimeContext::Local)
        .no_cascade()
        .load()
        .unwrap();

    // Only keys from .env.dev.local
    assert_eq!(vars.get("SECRET"), Some("local-secret"));
    assert_eq!(vars.get("DEV_NOTE"), Some("override"));
    // Base and .env.dev keys are NOT inherited
    assert_eq!(vars.get("PORT"), None);
    assert_eq!(vars.get("DB"), None);
    assert_eq!(vars.len(), 2);
}

#[test]
fn no_cascade_fixture_no_cascade_docker_picks_stage_base() {
    // no_cascade + dev + docker → .env.dev.docker doesn't exist, .env.dev.local is skipped
    // (Docker skips .local files), .env.docker doesn't exist
    // → highest existing is .env.dev
    let vars = EnvFlow::from_dir(fix("no-cascade"))
        .stage(Stage::Dev)
        .context(RuntimeContext::Docker)
        .no_cascade()
        .load()
        .unwrap();

    assert_eq!(vars.get("DB"), Some("dev-db"));
    assert_eq!(vars.get("DEBUG"), Some("true"));
    assert_eq!(vars.get("PORT"), None); // base not inherited
    assert_eq!(vars.len(), 2);
}

#[test]
fn no_cascade_fixture_no_cascade_no_stage_picks_base() {
    // no_cascade + no stage → only .env exists
    let vars = EnvFlow::from_dir(fix("no-cascade"))
        .no_cascade()
        .load()
        .unwrap();

    assert_eq!(vars.get("PORT"), Some("3000"));
    assert_eq!(vars.get("APP_NAME"), Some("myapp"));
    assert_eq!(vars.len(), 2);
}

// ── single-file fixture ──────────────────────────────────────────────────────
//
// Fixture layout:
//   .env          → PORT=3000, DB=base, APP_NAME=myapp
//   .env.override → DB=override-db, EXTRA=only-in-override

#[test]
fn single_file_from_file_loads_only_specified_file() {
    let vars = EnvFlow::from_file(fix("single-file").join(".env.override"))
        .load()
        .unwrap();

    // Only keys from .env.override
    assert_eq!(vars.get("DB"), Some("override-db"));
    assert_eq!(vars.get("EXTRA"), Some("only-in-override"));
    // Keys from .env are NOT present
    assert_eq!(vars.get("PORT"), None);
    assert_eq!(vars.get("APP_NAME"), None);
    assert_eq!(vars.len(), 2);
}

#[test]
fn single_file_from_file_base_env() {
    let vars = EnvFlow::from_file(fix("single-file").join(".env"))
        .load()
        .unwrap();

    assert_eq!(vars.get("PORT"), Some("3000"));
    assert_eq!(vars.get("DB"), Some("base"));
    assert_eq!(vars.get("APP_NAME"), Some("myapp"));
    // .env.override is NOT loaded
    assert_eq!(vars.get("EXTRA"), None);
    assert_eq!(vars.len(), 3);
}

#[test]
fn single_file_cascade_vs_no_cascade_vs_from_file() {
    // Shows the three modes side-by-side on the same fixture set

    // 1. Cascade (default from_dir): inherits PORT from .env, overrides DB
    //    Note: .env.override isn't a recognised naming convention so the resolver
    //    won't include it — use staged fixture to demonstrate cascade contrast
    let cascade = EnvFlow::from_dir(fix("staged"))
        .stage(Stage::Dev)
        .load()
        .unwrap();
    assert_eq!(cascade.get("APP_NAME"), Some("myapp")); // from .env
    assert_eq!(cascade.get("DB_HOST"), Some("dev-db")); // from .env.dev

    // 2. no_cascade on staged/dev → only .env.dev loaded
    let no_cascade = EnvFlow::from_dir(fix("staged"))
        .stage(Stage::Dev)
        .no_cascade()
        .load()
        .unwrap();
    assert_eq!(no_cascade.get("DB_HOST"), Some("dev-db")); // from .env.dev
    assert_eq!(no_cascade.get("APP_NAME"), None); // .env not loaded

    // 3. from_file → exactly one file
    let from_file = EnvFlow::from_file(fix("staged").join(".env"))
        .load()
        .unwrap();
    assert_eq!(from_file.get("APP_NAME"), Some("myapp")); // from .env only
    assert_eq!(from_file.get("DB_HOST"), Some("localhost")); // only base value
}

#[test]
fn reverse_local_env_style_loaded() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".env"), "PORT=3000\n").unwrap();
    std::fs::write(root.path().join(".local.env"), "DEBUG=true\n").unwrap();

    let vars = EnvFlow::from_dir(root.path())
        .context(RuntimeContext::Local)
        .load()
        .unwrap();

    assert_eq!(vars.get("PORT"), Some("3000"));
    assert_eq!(vars.get("DEBUG"), Some("true"));
}

#[test]
fn docker_compose_context_loads_docker_layers() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join(".env"), "APP=1\n").unwrap();
    std::fs::write(root.path().join(".env.docker"), "PORT=80\n").unwrap();

    let vars = EnvFlow::from_dir(root.path())
        .context(RuntimeContext::DockerCompose)
        .load()
        .unwrap();

    assert_eq!(vars.get("APP"), Some("1"));
    assert_eq!(vars.get("PORT"), Some("80"));
}

#[test]
fn orbstack_context_loads_orbstack_dir() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("orbstack")).unwrap();
    std::fs::write(root.path().join(".env"), "APP=1\n").unwrap();
    std::fs::write(root.path().join("orbstack/.env"), "PORT=8080\n").unwrap();

    let vars = EnvFlow::from_dir(root.path())
        .context(RuntimeContext::OrbStack)
        .load()
        .unwrap();

    assert_eq!(vars.get("PORT"), Some("8080"));
}
