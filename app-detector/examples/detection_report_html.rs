//! HTML detection fixture report — auto-opens in the browser.
//!
//! Run with:
//!   cargo run --example detection_report_html -p app-detector
//!
//! Optional: pass a custom fixtures path as the first argument.
//!   cargo run --example detection_report_html -p app-detector -- /my/fixtures

use app_detector::{
    engine::DetectionEngine,
    registry::StrategyRegistry,
    types::{DetectionData, DetectionReport},
};
use std::{env, fs, path::PathBuf};

// ─────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────

fn main() {
    let fixtures_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
        });

    if !fixtures_dir.exists() {
        eprintln!("Fixtures directory not found: {}", fixtures_dir.display());
        std::process::exit(1);
    }

    let mut dirs: Vec<PathBuf> = fs::read_dir(&fixtures_dir)
        .expect("read fixtures dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let engine = DetectionEngine::new(StrategyRegistry::with_defaults());

    // Run detection on all fixtures
    let results: Vec<FixtureResult> = dirs
        .iter()
        .map(|dir| {
            let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
            match engine.detect(dir) {
                Ok(report) => FixtureResult::Ok { name, report },
                Err(e)     => FixtureResult::Err { name, error: e.to_string() },
            }
        })
        .collect();

    let html = render_html(&results);

    // Write to temp file
    let out_path = std::env::temp_dir().join("app-detector-report.html");
    fs::write(&out_path, html).expect("write HTML report");

    println!("Report written to: {}", out_path.display());
    open_browser(&out_path);
}

// ─────────────────────────────────────────────────────────────
// Data model
// ─────────────────────────────────────────────────────────────

enum FixtureResult {
    Ok  { name: String, report: DetectionReport },
    Err { name: String, error: String },
}

struct Warning { fixture: String, message: String }

// ─────────────────────────────────────────────────────────────
// Open browser
// ─────────────────────────────────────────────────────────────

fn open_browser(path: &PathBuf) {
    let url = format!("file://{}", path.display());
    #[cfg(target_os = "macos")]
    { let _ = std::process::Command::new("open").arg(&url).spawn(); }
    #[cfg(target_os = "linux")]
    { let _ = std::process::Command::new("xdg-open").arg(&url).spawn(); }
    #[cfg(target_os = "windows")]
    { let _ = std::process::Command::new("cmd").args(["/c", "start", &url]).spawn(); }
}

// ─────────────────────────────────────────────────────────────
// HTML rendering
// ─────────────────────────────────────────────────────────────

fn render_html(results: &[FixtureResult]) -> String {
    let mut warnings: Vec<Warning> = Vec::new();

    // Gather warnings
    for r in results {
        if let FixtureResult::Ok { name, report } = r {
            collect_warnings(name, report, &mut warnings);
        }
        if let FixtureResult::Err { name, error } = r {
            warnings.push(Warning { fixture: name.clone(), message: format!("detection error: {error}") });
        }
    }

    let ok_count  = results.iter().filter(|r| matches!(r, FixtureResult::Ok { .. })).count();
    let warn_count = warnings.len();

    let cards_html: String = results.iter().map(render_card).collect();
    let table_html = render_summary_table(results);
    let warnings_html = render_warnings(&warnings);
    let date = chrono_now();

    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>App Detector — Fixture Report</title>
<style>
  :root {{
    --bg: #0f1117;
    --surface: #1a1d27;
    --surface2: #222535;
    --border: #2e3149;
    --text: #e2e4f0;
    --muted: #6b7099;
    --green: #4ade80;
    --yellow: #facc15;
    --red: #f87171;
    --blue: #60a5fa;
    --purple: #a78bfa;
    --cyan: #22d3ee;
    --orange: #fb923c;
    --badge-lang: #1d3461;
    --badge-lang-text: #60a5fa;
    --badge-mono: #2d1f61;
    --badge-mono-text: #a78bfa;
    --badge-svc: #1f3d2a;
    --badge-svc-text: #4ade80;
    --badge-docker: #1d3461;
    --badge-docker-text: #22d3ee;
    --badge-k8s: #2d3a1f;
    --badge-k8s-text: #86efac;
    --badge-local: #3a2d1f;
    --badge-local-text: #fb923c;
    --badge-warn: #3a2d1f;
    --badge-warn-text: #facc15;
  }}
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{
    background: var(--bg);
    color: var(--text);
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
    font-size: 13px;
    line-height: 1.6;
    padding: 2rem;
  }}
  a {{ color: var(--cyan); text-decoration: none; }}

  /* ── Header ── */
  header {{
    border-bottom: 1px solid var(--border);
    padding-bottom: 1.5rem;
    margin-bottom: 2rem;
  }}
  header h1 {{ font-size: 1.4rem; color: var(--cyan); letter-spacing: 0.05em; }}
  header .meta {{ color: var(--muted); font-size: 0.85rem; margin-top: 0.4rem; }}
  .counters {{ display: flex; gap: 1.5rem; margin-top: 1rem; }}
  .counter {{ background: var(--surface); border: 1px solid var(--border); border-radius: 8px;
              padding: 0.5rem 1rem; }}
  .counter .num {{ font-size: 1.3rem; font-weight: bold; }}
  .counter .label {{ color: var(--muted); font-size: 0.75rem; }}
  .counter.warn {{ border-color: #78350f; }}
  .counter.warn .num {{ color: var(--yellow); }}
  .counter.ok .num {{ color: var(--green); }}

  /* ── Nav tabs ── */
  nav {{ display: flex; gap: 0.5rem; margin-bottom: 2rem; flex-wrap: wrap; }}
  .tab {{ background: var(--surface); border: 1px solid var(--border); border-radius: 6px;
          padding: 0.3rem 0.8rem; cursor: pointer; color: var(--muted); font-size: 0.8rem;
          transition: all 0.15s; }}
  .tab:hover, .tab.active {{ border-color: var(--cyan); color: var(--cyan); background: #0e2030; }}

  /* ── Summary table ── */
  .section-title {{ font-size: 0.7rem; letter-spacing: 0.15em; text-transform: uppercase;
                    color: var(--muted); margin-bottom: 0.75rem; padding-bottom: 0.3rem;
                    border-bottom: 1px solid var(--border); }}
  table {{ width: 100%; border-collapse: collapse; margin-bottom: 2.5rem; }}
  th {{ text-align: left; color: var(--muted); font-size: 0.75rem; letter-spacing: 0.1em;
        text-transform: uppercase; padding: 0.5rem 0.8rem; border-bottom: 1px solid var(--border); }}
  td {{ padding: 0.5rem 0.8rem; border-bottom: 1px solid #1c1f30; vertical-align: top; }}
  tr:hover td {{ background: var(--surface); }}
  .fix-name {{ color: var(--text); font-weight: bold; }}
  .fix-name a {{ color: inherit; }}
  .fix-name a:hover {{ color: var(--cyan); }}
  .ws-count {{ color: var(--purple); font-weight: bold; }}
  .status-ok {{ color: var(--green); }}
  .status-warn {{ color: var(--yellow); }}
  .status-err {{ color: var(--red); }}

  /* ── Badges ── */
  .badge {{
    display: inline-block; border-radius: 4px; padding: 0.1rem 0.45rem;
    font-size: 0.75rem; font-weight: 600; margin: 0.15rem 0.15rem 0.15rem 0;
    white-space: nowrap;
  }}
  .badge-lang    {{ background: var(--badge-lang);   color: var(--badge-lang-text); }}
  .badge-mono    {{ background: var(--badge-mono);   color: var(--badge-mono-text); }}
  .badge-svc     {{ background: var(--badge-svc);    color: var(--badge-svc-text); }}
  .badge-docker  {{ background: var(--badge-docker); color: var(--badge-docker-text); }}
  .badge-k8s     {{ background: var(--badge-k8s);    color: var(--badge-k8s-text); }}
  .badge-local   {{ background: var(--badge-local);  color: var(--badge-local-text); }}
  .badge-warn    {{ background: var(--badge-warn);   color: var(--badge-warn-text); }}
  .badge-none    {{ background: #1a1d27; color: var(--muted); border: 1px solid var(--border); }}

  /* ── Cards ── */
  .cards {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(520px, 1fr)); gap: 1rem; margin-bottom: 2.5rem; }}
  .card {{
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    scroll-margin-top: 2rem;
  }}
  .card.has-warnings {{ border-color: #78350f; }}
  .card.has-error   {{ border-color: #7f1d1d; }}
  .card-header {{
    display: flex; align-items: baseline; justify-content: space-between;
    padding: 0.75rem 1rem;
    background: var(--surface2);
    border-bottom: 1px solid var(--border);
  }}
  .card-name {{ font-weight: bold; font-size: 0.95rem; color: var(--cyan); }}
  .card-path {{ color: var(--muted); font-size: 0.72rem; }}
  .card-body {{ padding: 0.75rem 1rem; }}
  .card-section {{ margin-bottom: 0.75rem; }}
  .card-section:last-child {{ margin-bottom: 0; }}
  .card-label {{ font-size: 0.68rem; text-transform: uppercase; letter-spacing: 0.12em;
                 color: var(--muted); margin-bottom: 0.3rem; }}

  /* ── Commands ── */
  .cmd-grid {{ display: flex; flex-wrap: wrap; gap: 0.25rem; }}
  .cmd {{
    background: #0f1117; border: 1px solid var(--border); border-radius: 4px;
    padding: 0.1rem 0.5rem; font-size: 0.72rem; color: var(--muted);
  }}
  .cmd .cmd-key {{ color: var(--text); }}
  .cmd.default-cmd {{ border-color: var(--orange); }}
  .cmd.default-cmd .cmd-key {{ color: var(--orange); }}

  /* ── Workspace tree ── */
  .ws-tree {{ border-left: 2px solid var(--border); margin-top: 0.5rem; padding-left: 0.75rem; }}
  .ws-item {{ margin-bottom: 0.5rem; }}
  .ws-item-name {{ color: var(--purple); font-weight: bold; font-size: 0.8rem; margin-bottom: 0.25rem; }}

  /* ── Detail rows ── */
  .detail-row {{ display: flex; gap: 0.5rem; align-items: baseline; margin: 0.1rem 0; }}
  .detail-key {{ color: var(--muted); font-size: 0.72rem; min-width: 90px; }}
  .detail-val {{ font-size: 0.78rem; color: var(--text); }}
  .detail-val.highlight {{ color: var(--green); }}

  /* ── Warnings section ── */
  .warnings-box {{
    border: 1px solid #78350f; background: #1c1207; border-radius: 8px;
    padding: 1rem 1.25rem; margin-bottom: 2rem;
  }}
  .warning-item {{
    display: flex; gap: 0.75rem; padding: 0.35rem 0;
    border-bottom: 1px solid #2a1c0a;
    font-size: 0.8rem;
  }}
  .warning-item:last-child {{ border-bottom: none; }}
  .warn-fix {{ color: var(--yellow); font-weight: bold; min-width: 160px; }}
  .warn-msg {{ color: #fde68a; }}

  .all-ok {{
    border: 1px solid #14532d; background: #0a1f12; border-radius: 8px;
    padding: 0.75rem 1.25rem; margin-bottom: 2rem;
    color: var(--green); font-size: 0.85rem;
  }}

  /* ── Section containers ── */
  .section {{ display: none; }}
  .section.active {{ display: block; }}
</style>
</head>
<body>

<header>
  <h1>App Detector — Detection Fixture Report</h1>
  <div class="meta">Generated {date} · {total} fixtures · {ok_count} ok · {warn_count} warning(s)</div>
  <div class="counters">
    <div class="counter ok">
      <div class="num">{ok_count}</div>
      <div class="label">fixtures ok</div>
    </div>
    <div class="counter warn">
      <div class="num">{warn_count}</div>
      <div class="label">warnings</div>
    </div>
  </div>
</header>

<nav>
  <div class="tab active" onclick="showSection('summary')">Summary Table</div>
  <div class="tab" onclick="showSection('cards')">Detail Cards</div>
  <div class="tab" onclick="showSection('warnings-section')">Warnings {warn_badge}</div>
</nav>

<div id="summary" class="section active">
  <div class="section-title">Overview — all fixtures</div>
  {table_html}
</div>

<div id="cards" class="section">
  <div class="section-title">Detail — per fixture</div>
  <div class="cards">
    {cards_html}
  </div>
</div>

<div id="warnings-section" class="section">
  <div class="section-title">Warnings & anomalies</div>
  {warnings_html}
</div>

<script>
function showSection(id) {{
  document.querySelectorAll('.section').forEach(s => s.classList.remove('active'));
  document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
  document.getElementById(id).classList.add('active');
  event.target.classList.add('active');
}}
// Deep link: clicking fixture name in table → go to card
function goToCard(id) {{
  document.querySelectorAll('.section').forEach(s => s.classList.remove('active'));
  document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
  document.getElementById('cards').classList.add('active');
  document.querySelectorAll('.tab')[1].classList.add('active');
  setTimeout(() => document.getElementById('card-' + id)?.scrollIntoView({{behavior:'smooth'}}), 50);
}}
</script>
</body>
</html>"#,
        date = date,
        total = results.len(),
        ok_count = ok_count,
        warn_count = warn_count,
        warn_badge = if warn_count > 0 { format!("({warn_count})") } else { String::new() },
        table_html = table_html,
        cards_html = cards_html,
        warnings_html = warnings_html,
    )
}

// ─────────────────────────────────────────────────────────────
// Summary table
// ─────────────────────────────────────────────────────────────

fn render_summary_table(results: &[FixtureResult]) -> String {
    let rows: String = results.iter().map(|r| match r {
        FixtureResult::Err { name, error } => format!(
            r#"<tr>
              <td class="fix-name">{name}</td>
              <td>—</td><td>—</td><td>—</td>
              <td class="status-err" title="{error}">✗ error</td>
            </tr>"#
        ),
        FixtureResult::Ok { name, report } => {
            let app_badges = app_type_badges(report);
            let env_badges = env_cap_badges(report);
            let ws = if report.children.is_empty() {
                r#"<span style="color:var(--muted)">—</span>"#.to_string()
            } else {
                format!(r#"<span class="ws-count">{}</span>"#, report.children.len())
            };
            let mut warns: Vec<Warning> = Vec::new();
            collect_warnings(name, report, &mut warns);
            let status = if warns.is_empty() {
                r#"<span class="status-ok">✓</span>"#.to_string()
            } else {
                format!(r#"<span class="status-warn" title="{}">⚠ {}</span>"#,
                    warns.iter().map(|w| &*w.message).collect::<Vec<_>>().join("; "),
                    warns.len())
            };
            format!(r#"<tr>
              <td class="fix-name"><a href="javascript:goToCard('{name}')">{name}</a></td>
              <td>{app_badges}</td>
              <td>{env_badges}</td>
              <td>{ws}</td>
              <td>{status}</td>
            </tr>"#)
        }
    }).collect();

    format!(r#"<table>
      <thead><tr>
        <th>Fixture</th>
        <th>App Types</th>
        <th>Env Capabilities</th>
        <th>Workspaces</th>
        <th>Status</th>
      </tr></thead>
      <tbody>{rows}</tbody>
    </table>"#)
}

fn app_type_badges(report: &DetectionReport) -> String {
    let types = report.app_types();
    if types.is_empty() {
        return r#"<span class="badge badge-none">none</span>"#.to_string();
    }
    types.iter().map(|r| {
        let (label, cls) = match &r.data {
            DetectionData::Language(i)  => (i.name.clone(),    "badge-lang"),
            DetectionData::Monorepo(i)  => (i.tool.clone(),    "badge-mono"),
            DetectionData::Service(i)   => (i.name.clone(),    "badge-svc"),
            _                           => (r.strategy_id.clone(), "badge-lang"),
        };
        format!(r#"<span class="badge {cls}">{label}</span>"#)
    }).collect()
}

fn env_cap_badges(report: &DetectionReport) -> String {
    let caps = report.env_capabilities();
    if caps.is_empty() {
        return r#"<span class="badge badge-none">none</span>"#.to_string();
    }
    caps.iter().map(|r| {
        let (label, cls) = match r.strategy_id.as_str() {
            "docker"         => ("docker",  "badge-docker"),
            "orbstack-env"   => ("orbstack","badge-docker"),
            "kubernetes-env" => ("k8s",     "badge-k8s"),
            "local-env"      => ("local",   "badge-local"),
            "nx-env"         => ("nx",      "badge-mono"),
            _                => (r.strategy_id.as_str(), "badge-lang"),
        };
        format!(r#"<span class="badge {cls}">{label}</span>"#)
    }).collect()
}

// ─────────────────────────────────────────────────────────────
// Detail cards
// ─────────────────────────────────────────────────────────────

fn render_card(result: &FixtureResult) -> String {
    match result {
        FixtureResult::Err { name, error } => format!(
            r#"<div class="card has-error" id="card-{name}">
              <div class="card-header">
                <span class="card-name">{name}</span>
                <span class="status-err">✗ error</span>
              </div>
              <div class="card-body">
                <div style="color:var(--red);font-size:0.8rem">{error}</div>
              </div>
            </div>"#
        ),
        FixtureResult::Ok { name, report } => {
            let mut warns: Vec<Warning> = Vec::new();
            collect_warnings(name, report, &mut warns);
            let has_warn = !warns.is_empty();

            let app_section = render_card_app_types(report);
            let env_section = render_card_env_caps(report);
            let ws_section  = render_card_workspaces(report);

            let warn_tag = if has_warn {
                format!(r#"<span class="badge badge-warn">⚠ {}</span>"#, warns.len())
            } else {
                r#"<span class="status-ok">✓</span>"#.to_string()
            };

            let path_display = report.path.display().to_string();
            let path_short = path_display
                .rsplit("tests/fixtures/")
                .next()
                .unwrap_or(&path_display);

            format!(r#"<div class="card{extra}" id="card-{name}">
              <div class="card-header">
                <div>
                  <span class="card-name">{name}</span>
                  <div class="card-path">tests/fixtures/{path_short}</div>
                </div>
                {warn_tag}
              </div>
              <div class="card-body">
                {app_section}
                {env_section}
                {ws_section}
              </div>
            </div>"#,
                extra = if has_warn { " has-warnings" } else { "" }
            )
        }
    }
}

fn render_card_app_types(report: &DetectionReport) -> String {
    let types = report.app_types();
    let label = r#"<div class="card-label">App Types</div>"#;
    if types.is_empty() {
        return format!(r#"{label}<div class="card-section"><span class="badge badge-none">none detected</span></div>"#);
    }
    let items: String = types.iter().map(|r| {
        let (name, version, extras) = match &r.data {
            DetectionData::Language(i) => {
                let v = i.version.clone().unwrap_or_default();
                let mut ex = Vec::new();
                if let Some(pm) = i.metadata.get("package_manager") {
                    ex.push(format!("pkg mgr: {}", pm.as_str().unwrap_or("")));
                }
                if let Some(ts) = i.metadata.get("typescript") {
                    if ts.as_bool() == Some(true) { ex.push("TypeScript".to_string()); }
                }
                (i.name.clone(), v, ex)
            }
            DetectionData::Monorepo(i) => {
                let v = i.version.clone().unwrap_or_default();
                let ex = vec![format!("{} workspaces", i.workspace_info.len())];
                (i.tool.clone(), v, ex)
            }
            DetectionData::Service(i) => {
                let v = i.version.clone().unwrap_or_default();
                (i.name.clone(), v, vec![])
            }
            _ => (r.strategy_id.clone(), String::new(), vec![]),
        };
        let badge_cls = match &r.data {
            DetectionData::Language(_) => "badge-lang",
            DetectionData::Monorepo(_) => "badge-mono",
            DetectionData::Service(_)  => "badge-svc",
            _ => "badge-lang",
        };
        let conf_tag = if r.confidence < 1.0 {
            format!(r#"<span class="badge badge-warn">{:.0}%</span>"#, r.confidence * 100.0)
        } else { String::new() };
        let extras_html: String = extras.iter()
            .map(|e| format!(r#"<span class="badge badge-none">{e}</span>"#))
            .collect();

        format!(r#"<div style="margin:0.25rem 0">
          <span class="badge {badge_cls}">{name}</span>
          {conf_tag}
          <span style="color:var(--muted);font-size:0.75rem">{version}</span>
          {extras_html}
        </div>"#)
    }).collect();

    format!(r#"<div class="card-section">{label}{items}</div>"#)
}

fn render_card_env_caps(report: &DetectionReport) -> String {
    let caps = report.env_capabilities();
    let label = r#"<div class="card-label">Env Capabilities</div>"#;
    if caps.is_empty() {
        return format!(r#"{label}<div class="card-section"><span class="badge badge-none">none</span></div>"#);
    }
    let items: String = caps.into_iter().map(render_env_cap_item).collect();
    format!(r#"<div class="card-section">{label}{items}</div>"#)
}

fn render_env_cap_item(r: &app_detector::types::DetectionResult) -> String {
    match &r.data {
        DetectionData::DockerEnv(info) => {
            let badge = r#"<span class="badge badge-docker">docker</span>"#;
            let mut details = Vec::new();
            if !info.dockerfiles.is_empty() {
                details.push(format!(r#"<div class="detail-row">
                  <span class="detail-key">Dockerfiles</span>
                  <span class="detail-val">{}</span></div>"#,
                    info.dockerfiles.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
                ));
            }
            if !info.stages.is_empty() {
                details.push(format!(r#"<div class="detail-row">
                  <span class="detail-key">Stages</span>
                  <span class="detail-val highlight">{}</span></div>"#,
                    info.stages.join(" → ")
                ));
            }
            if !info.compose_files.is_empty() {
                details.push(format!(r#"<div class="detail-row">
                  <span class="detail-key">Compose</span>
                  <span class="detail-val">{}</span></div>"#,
                    info.compose_files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
                ));
            }
            let cmds = render_commands(&info.commands, None);
            let warn = if info.commands.is_empty() {
                r#"<span class="badge badge-warn">⚠ no commands</span>"#
            } else { "" };
            format!(r#"<div style="margin:0.4rem 0">
              {badge} {warn}
              <div style="margin:0.3rem 0 0.3rem 0.5rem">{}</div>
              <div style="margin-left:0.5rem">{cmds}</div>
            </div>"#, details.join(""))
        }
        DetectionData::OrbStackEnv(info) => {
            let badge = r#"<span class="badge badge-docker">orbstack</span>"#;
            let cmds = render_commands(&info.commands, None);
            let warn = if info.commands.is_empty() {
                r#"<span class="badge badge-warn">⚠ no commands</span>"#
            } else { "" };
            format!(r#"<div style="margin:0.4rem 0">{badge} {warn}<div style="margin:0.3rem 0 0 0.5rem">{cmds}</div></div>"#)
        }
        DetectionData::LocalEnv(info) => {
            let badge = r#"<span class="badge badge-local">local</span>"#;
            let cmds = render_commands(&info.commands, info.suggested_default.as_deref());
            let warn = if info.commands.is_empty() {
                r#"<span class="badge badge-warn">⚠ no commands</span>"#
            } else { "" };
            format!(r#"<div style="margin:0.4rem 0">{badge} {warn}<div style="margin:0.3rem 0 0 0.5rem">{cmds}</div></div>"#)
        }
        DetectionData::KubernetesEnv(info) => {
            let badge = r#"<span class="badge badge-k8s">kubernetes</span>"#;
            let details = format!(
                r#"<div class="detail-row">
                  <span class="detail-key">Manifests</span>
                  <span class="detail-val">{}</span>
                </div>"#,
                info.manifests.len()
            );
            let cmds = render_commands(&info.commands, info.suggested_default.as_deref());
            let warn = if info.commands.is_empty() {
                r#"<span class="badge badge-warn">⚠ no commands</span>"#
            } else { "" };
            format!(r#"<div style="margin:0.4rem 0">
              {badge} {warn}
              <div style="margin:0.3rem 0 0.3rem 0.5rem">{details}</div>
              <div style="margin-left:0.5rem">{cmds}</div>
            </div>"#)
        }
        DetectionData::NxEnv(info) => {
            let badge = r#"<span class="badge badge-mono">nx</span>"#;
            let from_defaults = info.metadata.get("targets_from_defaults")
                .and_then(|v| v.as_u64()).unwrap_or(0);
            let from_projects = info.metadata.get("targets_from_projects")
                .and_then(|v| v.as_u64()).unwrap_or(0);
            let details = format!(
                r#"<div class="detail-row">
                  <span class="detail-key">Targets</span>
                  <span class="detail-val highlight">{}</span>
                </div>
                <div class="detail-row">
                  <span class="detail-key">Sources</span>
                  <span class="detail-val">{from_defaults} from targetDefaults · {from_projects} new from project.json</span>
                </div>"#,
                info.targets.join(", ")
            );
            let cmds = render_commands(&info.commands, info.suggested_default.as_deref());
            let warn = if info.commands.is_empty() {
                r#"<span class="badge badge-warn">⚠ no targets found in config</span>"#
            } else { "" };
            format!(r#"<div style="margin:0.4rem 0">
              {badge} {warn}
              <div style="margin:0.3rem 0 0.3rem 0.5rem">{details}</div>
              <div style="margin-left:0.5rem">{cmds}</div>
            </div>"#)
        }
        _ => format!(r#"<div style="margin:0.25rem 0"><span class="badge badge-none">{}</span></div>"#, r.strategy_id),
    }
}

fn render_commands(commands: &std::collections::HashMap<String, String>, default: Option<&str>) -> String {
    if commands.is_empty() { return String::new(); }
    let mut keys: Vec<&String> = commands.keys().collect();
    keys.sort();
    let items: String = keys.iter().map(|k| {
        let is_default = default == Some(k.as_str());
        let cls = if is_default { "cmd default-cmd" } else { "cmd" };
        let val = &commands[*k];
        format!(r#"<span class="{cls}" title="{val}"><span class="cmd-key">{k}</span></span>"#)
    }).collect();
    format!(r#"<div class="cmd-grid">{items}</div>"#)
}

fn render_card_workspaces(report: &DetectionReport) -> String {
    if report.children.is_empty() { return String::new(); }
    let label = r#"<div class="card-label">Workspaces</div>"#;
    let items: String = report.children.iter().map(|child| {
        let ws_name = child.path.file_name().unwrap_or_default().to_string_lossy();
        let app_badges = app_type_badges(child);
        let env_badges = env_cap_badges(child);
        // Show commands for each env cap in workspace
        let env_details: String = child.env_capabilities().into_iter().map(render_env_cap_item).collect();
        format!(r#"<div class="ws-item">
          <div class="ws-item-name">📦 {ws_name}</div>
          <div style="margin-left:0.5rem">
            <div style="margin-bottom:0.25rem">{app_badges} {env_badges}</div>
            {env_details}
          </div>
        </div>"#)
    }).collect();
    format!(r#"<div class="card-section">{label}<div class="ws-tree">{items}</div></div>"#)
}

// ─────────────────────────────────────────────────────────────
// Warnings panel
// ─────────────────────────────────────────────────────────────

fn render_warnings(warnings: &[Warning]) -> String {
    if warnings.is_empty() {
        return r#"<div class="all-ok">✓ No warnings — all fixtures look correct.</div>"#.to_string();
    }
    let items: String = warnings.iter().map(|w| {
        format!(r#"<div class="warning-item">
          <span class="warn-fix">{}</span>
          <span class="warn-msg">{}</span>
        </div>"#, w.fixture, w.message)
    }).collect();
    format!(r#"<div class="warnings-box">{items}</div>"#)
}

// ─────────────────────────────────────────────────────────────
// Warning collection
// ─────────────────────────────────────────────────────────────

fn collect_warnings(name: &str, report: &DetectionReport, warnings: &mut Vec<Warning>) {
    for r in report.env_capabilities() {
        let has_commands = match &r.data {
            DetectionData::DockerEnv(i)     => !i.commands.is_empty(),
            DetectionData::OrbStackEnv(i)   => !i.commands.is_empty(),
            DetectionData::LocalEnv(i)      => !i.commands.is_empty(),
            DetectionData::KubernetesEnv(i) => !i.commands.is_empty(),
            DetectionData::NxEnv(i)         => !i.commands.is_empty(),
            _ => true,
        };
        if !has_commands {
            warnings.push(Warning {
                fixture: name.to_string(),
                message: format!("env cap '{}' has no commands", r.strategy_id),
            });
        }
        if r.confidence < 0.8 {
            warnings.push(Warning {
                fixture: name.to_string(),
                message: format!("low confidence on '{}' ({:.0}%)", r.strategy_id, r.confidence * 100.0),
            });
        }
    }
    for r in report.app_types() {
        if r.confidence < 0.8 {
            warnings.push(Warning {
                fixture: name.to_string(),
                message: format!("low confidence on '{}' ({:.0}%)", r.strategy_id, r.confidence * 100.0),
            });
        }
    }
    for child in &report.children {
        let ws_name = child.path.file_name().unwrap_or_default().to_string_lossy();
        for r in child.env_capabilities() {
            let has_commands = match &r.data {
                DetectionData::DockerEnv(i)     => !i.commands.is_empty(),
                DetectionData::OrbStackEnv(i)   => !i.commands.is_empty(),
                DetectionData::LocalEnv(i)      => !i.commands.is_empty(),
                DetectionData::KubernetesEnv(i) => !i.commands.is_empty(),
                _ => true,
            };
            if !has_commands {
                warnings.push(Warning {
                    fixture: format!("{name}/{ws_name}"),
                    message: format!("env cap '{}' has no commands", r.strategy_id),
                });
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────
// Utilities
// ─────────────────────────────────────────────────────────────

fn chrono_now() -> String {
    // Simple timestamp without chrono dependency
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let s = secs % 86400;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    // Days since epoch → approximate date (good enough for a report header)
    let days = secs / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{sec:02} UTC")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut y = 1970u64;
    loop {
        let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
        let dy = if leap { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let months = [31u64, if leap {29} else {28}, 31,30,31,30,31,31,30,31,30,31];
    let mut mo = 1u64;
    for dm in months {
        if days < dm { break; }
        days -= dm;
        mo += 1;
    }
    (y, mo, days + 1)
}
