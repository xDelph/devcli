/// Integration tests using the on-disk fixture directories.
use env_flow::{EnvFlow, EnvVars, LayerType, RuntimeContext, Stage};
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
    let vars = EnvFlow::from_dir(fix("edge-cases/unicode"))
        .load()
        .unwrap();
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
    assert!(existing.iter().any(|l| l.layer_type == LayerType::StageBase));
}
