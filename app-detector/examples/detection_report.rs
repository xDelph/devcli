//! Detection fixture report
//!
//! Runs the detection engine on every directory inside `tests/fixtures/` and
//! renders a human-readable report so you can quickly verify that each fixture
//! is detected correctly and spot any regressions.
//!
//! Run with:
//!   cargo run --example detection_report -p app-detector
//!
//! Optional: pass a custom fixtures path as the first argument.
//!   cargo run --example detection_report -p app-detector -- /my/fixtures

use app_detector::{
    engine::DetectionEngine,
    registry::StrategyRegistry,
    types::{DetectionData, DetectionReport},
};
use std::{env, path::PathBuf};

// ──────────────────────────────────────────────────────────────────────────────
// Colour helpers (no external crate needed — ANSI only)
// ──────────────────────────────────────────────────────────────────────────────

fn bold(s: &str) -> String { format!("\x1b[1m{s}\x1b[0m") }
fn dim(s: &str)  -> String { format!("\x1b[2m{s}\x1b[0m") }
fn green(s: &str) -> String { format!("\x1b[32m{s}\x1b[0m") }
fn yellow(s: &str) -> String { format!("\x1b[33m{s}\x1b[0m") }
fn red(s: &str)   -> String { format!("\x1b[31m{s}\x1b[0m") }
fn cyan(s: &str)  -> String { format!("\x1b[36m{s}\x1b[0m") }

// ──────────────────────────────────────────────────────────────────────────────
// Report structs
// ──────────────────────────────────────────────────────────────────────────────

/// One row in the summary table
struct FixtureRow {
    name: String,
    app_types: Vec<String>,
    env_caps: Vec<String>,
    workspace_count: usize,
    warnings: Vec<String>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Main
// ──────────────────────────────────────────────────────────────────────────────

fn main() {
    let fixtures_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
        });

    if !fixtures_dir.exists() {
        eprintln!("{} Fixtures directory not found: {}", red("✗"), fixtures_dir.display());
        std::process::exit(1);
    }

    // Collect fixture dirs, sorted alphabetically
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&fixtures_dir)
        .expect("read fixtures dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let engine = DetectionEngine::new(StrategyRegistry::with_defaults());

    print_header(dirs.len());

    let mut rows: Vec<FixtureRow> = Vec::new();
    let mut global_warnings: Vec<String> = Vec::new();

    for dir in &dirs {
        let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();

        match engine.detect(dir) {
            Err(e) => {
                println!("\n{} {} — {}", red("✗"), bold(&name), red(&e.to_string()));
                global_warnings.push(format!("{name}: detection error — {e}"));
            }
            Ok(report) => {
                let row = render_fixture(&name, &report);
                for w in &row.warnings {
                    global_warnings.push(format!("{name}: {w}"));
                }
                rows.push(row);
                print_fixture_section(&name, &report);
            }
        }
    }

    print_summary_table(&rows);
    print_warnings(&global_warnings);
    print_footer(rows.len(), global_warnings.len());
}

// ──────────────────────────────────────────────────────────────────────────────
// Sections
// ──────────────────────────────────────────────────────────────────────────────

fn print_header(count: usize) {
    let title = "  App Detector — Detection Fixture Report  ";
    let sub   = format!("  {} fixtures  ", count);
    let w     = title.len().max(sub.len()) + 4;
    let line  = "═".repeat(w);

    println!();
    println!("{}", cyan(&format!("╔{line}╗")));
    println!("{}", cyan(&format!("║{title:^w$}║", title = bold(title.trim()))));
    println!("{}", cyan(&format!("║{sub:^w$}║", sub = dim(sub.trim()))));
    println!("{}", cyan(&format!("╚{line}╝")));
    println!();
}

fn print_fixture_section(name: &str, report: &DetectionReport) {
    let bar = "─".repeat(60usize.saturating_sub(name.len() + 2));
    println!("{} {}", bold(&cyan(name)), dim(&bar));

    let app_types = report.app_types();
    if app_types.is_empty() {
        println!("  {}", dim("App Types:    (none)"));
    } else {
        print!("  App Types:   ");
        let parts: Vec<String> = app_types.iter().map(|r| format_app_type(r)).collect();
        println!("{}", parts.join("  "));
    }

    let env_caps = report.env_capabilities();
    if env_caps.is_empty() {
        println!("  {}", dim("Env Caps:     (none)"));
    } else {
        println!("  Env Caps:");
        for r in &env_caps {
            let (cap_str, warn) = format_env_cap(r);
            if let Some(w) = warn {
                println!("    {} {}", cap_str, yellow(&format!("⚠  {w}")));
            } else {
                println!("    {}", cap_str);
            }
        }
    }

    if !report.children.is_empty() {
        println!("  Workspaces:  ({} found)", report.children.len());
        for child in &report.children {
            let child_name = child.path.file_name().unwrap_or_default().to_string_lossy();
            let app_str = child.app_types().iter()
                .map(|r| format_app_type(r))
                .collect::<Vec<_>>()
                .join(", ");
            let env_str = child.env_capabilities().iter()
                .map(|r| r.strategy_id.clone())
                .collect::<Vec<_>>()
                .join(", ");
            println!("    {} {}", cyan(&format!("┌─ {child_name}")), dim(&format!("[{app_str}] + [{env_str}]")));
        }
    }

    println!();
}

fn format_app_type(r: &app_detector::types::DetectionResult) -> String {
    let detail = match &r.data {
        DetectionData::Language(info) => {
            let v = info.version.as_deref().unwrap_or("?");
            format!("{} {}", info.name, v)
        }
        DetectionData::Monorepo(info) => {
            format!("{} ({} workspaces)", info.tool, info.workspace_info.len())
        }
        DetectionData::Service(info) => info.name.clone(),
        _ => r.strategy_id.clone(),
    };
    let conf = if r.confidence < 1.0 {
        format!(" {}", yellow(&format!("{:.0}%", r.confidence * 100.0)))
    } else {
        String::new()
    };
    format!("{}{conf}", green(&detail))
}

/// Returns (formatted string, optional warning message)
fn format_env_cap(r: &app_detector::types::DetectionResult) -> (String, Option<String>) {
    match &r.data {
        DetectionData::DockerEnv(info) => {
            let mut parts = Vec::new();
            if !info.dockerfiles.is_empty() {
                parts.push(format!("{} Dockerfile(s)", info.dockerfiles.len()));
            }
            if !info.compose_files.is_empty() {
                parts.push(format!("{} compose", info.compose_files.len()));
            }
            if !info.stages.is_empty() {
                parts.push(format!("stages: {}", info.stages.join(", ")));
            }
            let cmd_keys: Vec<&String> = info.commands.keys().collect();
            let cmd_str = format!("[{}]", cmd_keys.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            let label = bold(&format!("{:<14}", r.strategy_id));
            let detail = dim(&parts.join(" · "));
            let warn = if info.commands.is_empty() { Some("no commands generated".to_string()) } else { None };
            (format!("{} {} {}", label, cmd_str, detail), warn)
        }
        DetectionData::OrbStackEnv(info) => {
            let cmd_keys: Vec<&String> = info.commands.keys().collect();
            let cmd_str = format!("[{}]", cmd_keys.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            let warn = if info.commands.is_empty() { Some("no commands generated".to_string()) } else { None };
            (format!("{} {}", bold(&format!("{:<14}", r.strategy_id)), cmd_str), warn)
        }
        DetectionData::LocalEnv(info) => {
            let cmd_keys: Vec<&String> = info.commands.keys().collect();
            let default_tag = info.suggested_default.as_deref()
                .map(|d| dim(&format!(" → {d}")))
                .unwrap_or_default();
            let cmd_str = format!("[{}]", cmd_keys.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            let warn = if info.commands.is_empty() { Some("no commands (no scripts?)".to_string()) } else { None };
            (format!("{} {}{}", bold(&format!("{:<14}", r.strategy_id)), cmd_str, default_tag), warn)
        }
        DetectionData::KubernetesEnv(info) => {
            let manifest_count = info.manifests.len();
            let helm_count = info.helm_charts.len();
            let cmd_keys: Vec<&String> = info.commands.keys().collect();
            let cmd_str = format!("[{}]", cmd_keys.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            let detail = dim(&format!("{manifest_count} manifests, {helm_count} charts"));
            let warn = if info.commands.is_empty() { Some("no commands generated".to_string()) } else { None };
            (format!("{} {} {}", bold(&format!("{:<14}", r.strategy_id)), cmd_str, detail), warn)
        }
        DetectionData::NxEnv(info) => {
            let default_tag = info.suggested_default.as_deref()
                .map(|d| dim(&format!(" → {d}")))
                .unwrap_or_default();
            let targets_str = format!("[{}]", info.targets.join(", "));
            let detail = dim(&format!("{} targets (A:{} B:{})",
                info.targets.len(),
                info.metadata.get("targets_from_defaults").and_then(|v| v.as_u64()).unwrap_or(0),
                info.metadata.get("targets_from_projects").and_then(|v| v.as_u64()).unwrap_or(0),
            ));
            let warn = if info.commands.is_empty() { Some("no targets found in config".to_string()) } else { None };
            (format!("{} {}{} {}", bold(&format!("{:<14}", r.strategy_id)), targets_str, default_tag, detail), warn)
        }
        _ => {
            let label = bold(&format!("{:<14}", r.strategy_id));
            let conf = if r.confidence < 1.0 {
                format!(" {}", yellow(&format!("{:.0}%", r.confidence * 100.0)))
            } else {
                String::new()
            };
            (format!("{}{conf}", label), None)
        }
    }
}

fn render_fixture(name: &str, report: &DetectionReport) -> FixtureRow {
    let mut warnings = Vec::new();

    let app_types: Vec<String> = report.app_types().iter().map(|r| {
        if r.confidence < 0.8 {
            warnings.push(format!("low confidence on {} ({:.0}%)", r.strategy_id, r.confidence * 100.0));
        }
        r.strategy_id.clone()
    }).collect();

    let env_caps: Vec<String> = report.env_capabilities().iter().map(|r| {
        let has_commands = match &r.data {
            DetectionData::DockerEnv(i)     => !i.commands.is_empty(),
            DetectionData::OrbStackEnv(i)   => !i.commands.is_empty(),
            DetectionData::LocalEnv(i)      => !i.commands.is_empty(),
            DetectionData::KubernetesEnv(i) => !i.commands.is_empty(),
            DetectionData::NxEnv(i)         => !i.commands.is_empty(),
            _ => true,
        };
        if !has_commands {
            warnings.push(format!("env cap '{}' has no commands", r.strategy_id));
        }
        r.strategy_id.clone()
    }).collect();

    // Check workspace children too
    for child in &report.children {
        for r in child.env_capabilities() {
            let has_commands = match &r.data {
                DetectionData::DockerEnv(i)     => !i.commands.is_empty(),
                DetectionData::OrbStackEnv(i)   => !i.commands.is_empty(),
                DetectionData::LocalEnv(i)      => !i.commands.is_empty(),
                DetectionData::KubernetesEnv(i) => !i.commands.is_empty(),
                DetectionData::NxEnv(i)         => !i.commands.is_empty(),
                _ => true,
            };
            if !has_commands {
                let ws_name = child.path.file_name().unwrap_or_default().to_string_lossy();
                warnings.push(format!("workspace '{ws_name}' env cap '{}' has no commands", r.strategy_id));
            }
        }
    }

    FixtureRow {
        name: name.to_string(),
        app_types,
        env_caps,
        workspace_count: report.children.len(),
        warnings,
    }
}

fn print_summary_table(rows: &[FixtureRow]) {
    let name_w = rows.iter().map(|r| r.name.len()).max().unwrap_or(10).max(10);
    let type_w = rows.iter()
        .map(|r| if r.app_types.is_empty() { 6 } else { r.app_types.join(", ").len() })
        .max().unwrap_or(10).max(10);
    let env_w  = rows.iter()
        .map(|r| if r.env_caps.is_empty() { 6 } else { r.env_caps.join(", ").len() })
        .max().unwrap_or(12).max(12);

    let sep = format!("{}┼{}┼{}┼{}", "─".repeat(name_w + 2), "─".repeat(type_w + 2), "─".repeat(env_w + 2), "─".repeat(11));

    println!("{}", bold("Summary Table"));
    println!("{}", "═".repeat(sep.len()));
    println!(" {:<name_w$}  {:<type_w$}  {:<env_w$}  Workspaces",
        bold("Fixture"), bold("App Types"), bold("Env Capabilities"));
    println!("{sep}");

    for row in rows {
        let types_str = if row.app_types.is_empty() { dim("—") } else { green(&row.app_types.join(", ")) };
        let env_str   = if row.env_caps.is_empty()  { dim("—") } else { row.env_caps.join(", ") };
        let ws_str    = if row.workspace_count == 0 { dim("—") } else { yellow(&row.workspace_count.to_string()) };
        let warn_tag  = if row.warnings.is_empty() { green("✓") } else { red(&format!("⚠ {}", row.warnings.len())) };

        println!(" {:<name_w$}  {:<type_w$}  {:<env_w$}  {:<10}  {}",
            row.name, types_str, env_str, ws_str, warn_tag);
    }

    println!("{sep}");
    println!();
}

fn print_warnings(warnings: &[String]) {
    if warnings.is_empty() {
        println!("{}", green("No warnings — all fixtures look good."));
    } else {
        println!("{}", bold(&yellow(&format!("Warnings ({}):", warnings.len()))));
        for w in warnings {
            println!("  {} {w}", yellow("⚠"));
        }
    }
    println!();
}

fn print_footer(count: usize, warning_count: usize) {
    let status = if warning_count == 0 {
        green(&format!("{count} fixtures OK"))
    } else {
        red(&format!("{count} fixtures · {warning_count} warning(s)"))
    };
    println!("{}", bold(&status));
}
