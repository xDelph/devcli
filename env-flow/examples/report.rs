//! HTML env-flow fixture report — auto-opens in the browser.
//!
//! Run with:
//!   cargo run --example report -p env-flow
//!
//! Optional: pass a custom directory as the first argument.
//!   cargo run --example report -p env-flow -- /my/project

use env_flow::{
    EnvFlow, EnvVars, LayerType, RuntimeContext, Stage,
    detector,
    types::ResolvedLayer,
};
use std::{env, fs, path::{Path, PathBuf}};

// ─────────────────────────────────────────────────────────────
// Data model
// ─────────────────────────────────────────────────────────────

struct FixtureResult {
    name: String,
    path: PathBuf,
    file_count: usize,
    stages: Vec<String>,
    contexts: Vec<String>,
    scenarios: Vec<ScenarioResult>,
    warnings: Vec<String>,
}

#[derive(Clone, PartialEq)]
enum LoadMode {
    /// Default: all layers merged from lowest to highest priority.
    Cascade,
    /// Only the single highest-priority existing layer is loaded.
    NoCascade,
    /// A specific file loaded directly, bypassing the layer chain.
    FromFile,
}

impl LoadMode {
    fn label(&self) -> &'static str {
        match self {
            LoadMode::Cascade   => "cascade",
            LoadMode::NoCascade => "no-cascade",
            LoadMode::FromFile  => "from-file",
        }
    }
    fn css_class(&self) -> &'static str {
        match self {
            LoadMode::Cascade   => "badge-mode-cascade",
            LoadMode::NoCascade => "badge-mode-nocascade",
            LoadMode::FromFile  => "badge-mode-fromfile",
        }
    }
}

struct ScenarioResult {
    label: String,
    mode: LoadMode,
    layers: Vec<ResolvedLayer>,
    vars: Result<EnvVars, env_flow::Error>,
}

struct Warning { fixture: String, message: String }

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
        eprintln!("Directory not found: {}", fixtures_dir.display());
        std::process::exit(1);
    }

    // Collect fixture dirs (immediate children that are directories)
    let mut dirs: Vec<PathBuf> = fs::read_dir(&fixtures_dir)
        .expect("read dir")
        .flatten()
        .flat_map(|e| {
            let p = e.path();
            if p.is_dir() {
                // Also include immediate sub-dirs (edge-cases/*)
                let sub: Vec<PathBuf> = fs::read_dir(&p)
                    .ok()
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect();
                if sub.is_empty() {
                    vec![p]
                } else {
                    // Has sub-dirs: include both parent and sub-dirs
                    let mut all = vec![p];
                    all.extend(sub);
                    all
                }
            } else {
                vec![]
            }
        })
        .collect();
    dirs.sort();

    let results: Vec<FixtureResult> = dirs.iter().map(|dir| analyse(dir, &fixtures_dir)).collect();
    let html = render_html(&results, &fixtures_dir);

    let out_path = std::env::temp_dir().join("env-flow-report.html");
    fs::write(&out_path, html).expect("write HTML");
    println!("Report written to: {}", out_path.display());
    open_browser(&out_path);
}

// ─────────────────────────────────────────────────────────────
// Analysis
// ─────────────────────────────────────────────────────────────

fn analyse(dir: &Path, root: &Path) -> FixtureResult {
    let name = dir
        .strip_prefix(root)
        .unwrap_or(dir)
        .to_string_lossy()
        .replace('\\', "/");

    // Discover env files
    let detected = detector::discover(dir).unwrap_or_default();
    let file_count = detected.len();

    // Extract unique stages and contexts from discovered files
    let mut stages: Vec<String> = detected.iter()
        .filter_map(|f| f.stage.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut contexts: Vec<String> = detected.iter()
        .filter_map(|f| f.context.clone())
        .filter(|c| c != "local") // 'local' is the default, show separately
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    if stages.is_empty() { stages.push("(none)".into()); }
    if contexts.is_empty() { contexts.push("local".into()); }

    // Build representative scenarios based on what's in the fixture
    let mut scenarios = Vec::new();
    let actual_stages: Vec<Option<Stage>> = {
        let mut s: Vec<Option<Stage>> = detected.iter()
            .filter_map(|f| f.stage.as_deref())
            .map(|s| Some(Stage::from(s)))
            .collect::<std::collections::BTreeSet<_>>() // deduplicate via Ord
            .into_iter()
            .collect();
        s.insert(0, None); // always include "no stage"
        s.dedup();
        s
    };

    let probe_contexts = build_probe_contexts(&detected);

    for ctx in &probe_contexts {
        for stage in &actual_stages {
            let stage_label = stage.as_ref().map(|s| s.as_str()).unwrap_or("base");
            let base_label  = format!("{stage_label} / {}", ctx.as_str());

            let layers = match EnvFlow::from_dir(dir)
                .context(ctx.clone())
                .stage_opt(stage.clone())
                .layers()
            {
                Ok(l) => l,
                Err(_) => continue,
            };

            // Only include scenarios where at least one file exists
            if !layers.iter().any(|l| l.exists) {
                continue;
            }

            // ── Cascade (default) ──────────────────────────────────────────
            let vars_cascade = EnvFlow::from_dir(dir)
                .context(ctx.clone())
                .stage_opt(stage.clone())
                .load();
            scenarios.push(ScenarioResult {
                label: base_label.clone(),
                mode: LoadMode::Cascade,
                layers: layers.clone(),
                vars: vars_cascade,
            });

            // ── No-cascade: only if there are 2+ existing layers ───────────
            let existing_count = layers.iter().filter(|l| l.exists).count();
            if existing_count >= 2 {
                let vars_nc = EnvFlow::from_dir(dir)
                    .context(ctx.clone())
                    .stage_opt(stage.clone())
                    .no_cascade()
                    .load();
                scenarios.push(ScenarioResult {
                    label: format!("{base_label} [no-cascade]"),
                    mode: LoadMode::NoCascade,
                    layers: layers.clone(),
                    vars: vars_nc,
                });
            }

            // ── From-file: show each existing layer individually ───────────
            // Only do this for fixtures with <= 4 existing files to keep report concise
            if existing_count <= 4 {
                for layer in layers.iter().filter(|l| l.exists) {
                    let file_label = format!(
                        "{} [from-file: {}]",
                        base_label,
                        layer.relative_path
                    );
                    let vars_ff = EnvFlow::from_file(&layer.path).load();
                    // Reuse single-element layer list for the from-file view
                    scenarios.push(ScenarioResult {
                        label: file_label,
                        mode: LoadMode::FromFile,
                        layers: vec![layer.clone()],
                        vars: vars_ff,
                    });
                }
            }
        }
    }

    // Collect warnings
    let mut warnings = Vec::new();
    if file_count == 0 {
        warnings.push("no .env files found".into());
    }
    for scenario in &scenarios {
        if let Err(e) = &scenario.vars {
            warnings.push(format!("[{}] load error: {}", scenario.label, e));
        }
    }

    FixtureResult { name, path: dir.to_path_buf(), file_count, stages, contexts, scenarios, warnings }
}

/// Build a deduplicated list of runtime contexts to probe, based on what files exist.
fn build_probe_contexts(detected: &[detector::DetectedFile]) -> Vec<RuntimeContext> {
    use std::collections::BTreeSet;

    let mut ctx_names: BTreeSet<String> = BTreeSet::new();
    ctx_names.insert("local".into()); // always probe local

    for f in detected {
        if let Some(ref c) = f.context {
            if c != "local" {
                ctx_names.insert(c.clone());
            }
        }
    }

    ctx_names.into_iter().map(|s| RuntimeContext::from(s.as_str())).collect()
}

// ─────────────────────────────────────────────────────────────
// Browser
// ─────────────────────────────────────────────────────────────

fn open_browser(path: &std::path::Path) {
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

fn render_html(results: &[FixtureResult], fixtures_dir: &Path) -> String {
    let mut warnings: Vec<Warning> = Vec::new();
    for r in results {
        for w in &r.warnings {
            warnings.push(Warning { fixture: r.name.clone(), message: w.clone() });
        }
    }

    let ok_count = results.iter().filter(|r| r.warnings.is_empty()).count();
    let warn_count = warnings.len();
    let total = results.len();
    let date = now_utc();
    let fixture_path = fixtures_dir.display();

    let table_html = render_summary_table(results);
    let cards_html: String = results.iter().map(render_card).collect();
    let warnings_html = render_warnings(&warnings);
    let mode_comparison_html = render_mode_comparison(results);
    let warn_badge = if warn_count > 0 { format!("({warn_count})") } else { String::new() };

    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>env-flow — Fixture Report</title>
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
    --teal: #2dd4bf;
    /* layer-type colours */
    --layer-base:        #1d3461; --layer-base-t:    #60a5fa;
    --layer-ctx:         #2d1f61; --layer-ctx-t:     #a78bfa;
    --layer-stage:       #1f3d2a; --layer-stage-t:   #4ade80;
    --layer-stagectx:    #1d3a3a; --layer-stagectx-t:#2dd4bf;
    --layer-local:       #3a2d1f; --layer-local-t:   #fb923c;
    --layer-stagelocal:  #3a1f1f; --layer-stagelocal-t:#f87171;
    --layer-missing:     #1a1d27; --layer-missing-t:  #6b7099;
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

  header {{
    border-bottom: 1px solid var(--border);
    padding-bottom: 1.5rem;
    margin-bottom: 2rem;
  }}
  header h1 {{ font-size: 1.4rem; color: var(--cyan); letter-spacing: 0.05em; }}
  header .meta {{ color: var(--muted); font-size: 0.85rem; margin-top: 0.4rem; }}
  .counters {{ display: flex; gap: 1.5rem; margin-top: 1rem; flex-wrap: wrap; }}
  .counter {{
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 8px; padding: 0.5rem 1rem;
  }}
  .counter .num {{ font-size: 1.3rem; font-weight: bold; }}
  .counter .label {{ color: var(--muted); font-size: 0.75rem; }}
  .counter.warn {{ border-color: #78350f; }}
  .counter.warn .num {{ color: var(--yellow); }}
  .counter.ok .num {{ color: var(--green); }}

  nav {{
    display: flex; gap: 0.5rem; margin-bottom: 2rem; flex-wrap: wrap;
  }}
  .tab {{
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 6px; padding: 0.3rem 0.8rem; cursor: pointer;
    color: var(--muted); font-size: 0.8rem; transition: all 0.15s;
  }}
  .tab:hover, .tab.active {{
    border-color: var(--cyan); color: var(--cyan); background: #0e2030;
  }}

  .section-title {{
    font-size: 0.7rem; letter-spacing: 0.15em; text-transform: uppercase;
    color: var(--muted); margin-bottom: 0.75rem; padding-bottom: 0.3rem;
    border-bottom: 1px solid var(--border);
  }}
  table {{ width: 100%; border-collapse: collapse; margin-bottom: 2.5rem; }}
  th {{
    text-align: left; color: var(--muted); font-size: 0.75rem;
    letter-spacing: 0.1em; text-transform: uppercase;
    padding: 0.5rem 0.8rem; border-bottom: 1px solid var(--border);
  }}
  td {{ padding: 0.5rem 0.8rem; border-bottom: 1px solid #1c1f30; vertical-align: top; }}
  tr:hover td {{ background: var(--surface); }}
  .fix-name {{ color: var(--text); font-weight: bold; }}
  .fix-name a {{ color: inherit; }}
  .fix-name a:hover {{ color: var(--cyan); }}
  .status-ok {{ color: var(--green); }}
  .status-warn {{ color: var(--yellow); }}
  .status-err {{ color: var(--red); }}

  /* badges */
  .badge {{
    display: inline-block; border-radius: 4px; padding: 0.1rem 0.45rem;
    font-size: 0.72rem; font-weight: 600; margin: 0.1rem 0.1rem 0.1rem 0;
    white-space: nowrap;
  }}
  .badge-stage    {{ background: #1f3d2a; color: #4ade80; }}
  .badge-ctx      {{ background: #2d1f61; color: #a78bfa; }}
  .badge-file     {{ background: #1d3461; color: #60a5fa; }}
  .badge-none     {{ background: #1a1d27; color: var(--muted); border: 1px solid var(--border); }}
  .badge-warn     {{ background: #3a2d1f; color: var(--yellow); }}
  .badge-ok       {{ background: #1f3d2a; color: #4ade80; }}
  .badge-err      {{ background: #3d1f1f; color: var(--red); }}
  /* load mode badges */
  .badge-mode-cascade    {{ background: #1d3461; color: #60a5fa; }}
  .badge-mode-nocascade  {{ background: #3a2d1f; color: #fb923c; }}
  .badge-mode-fromfile   {{ background: #2d1f61; color: #a78bfa; }}

  /* layer pills */
  .layer-row {{
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.25rem 0; font-size: 0.75rem;
    border-bottom: 1px solid #1c1f30;
  }}
  .layer-row:last-child {{ border-bottom: none; }}
  .layer-pill {{
    border-radius: 4px; padding: 0.1rem 0.4rem;
    font-size: 0.68rem; font-weight: 600; white-space: nowrap; flex-shrink: 0;
  }}
  .lp-Base          {{ background: var(--layer-base);       color: var(--layer-base-t); }}
  .lp-ContextBase   {{ background: var(--layer-ctx);        color: var(--layer-ctx-t); }}
  .lp-StageBase     {{ background: var(--layer-stage);      color: var(--layer-stage-t); }}
  .lp-StageContext  {{ background: var(--layer-stagectx);   color: var(--layer-stagectx-t); }}
  .lp-LocalOverride {{ background: var(--layer-local);      color: var(--layer-local-t); }}
  .lp-StageLocalOverride {{ background: var(--layer-stagelocal); color: var(--layer-stagelocal-t); }}
  .layer-path {{ color: var(--text); }}
  .layer-path.missing {{ color: var(--muted); text-decoration: line-through; }}
  .layer-check {{ color: var(--green); }}
  .layer-dash  {{ color: var(--muted); }}

  /* vars table */
  .vars-table {{ width: 100%; font-size: 0.72rem; border-collapse: collapse; margin-top: 0.3rem; }}
  .vars-table td {{ padding: 0.15rem 0.4rem; vertical-align: top; }}
  .var-key {{ color: var(--cyan); font-weight: 600; white-space: nowrap; }}
  .var-val {{ color: var(--text); word-break: break-all; }}
  .var-src {{ color: var(--muted); font-size: 0.65rem; }}

  /* cards */
  .cards {{
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(600px, 1fr));
    gap: 1rem; margin-bottom: 2.5rem;
  }}
  .card {{
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 10px; overflow: hidden; scroll-margin-top: 2rem;
  }}
  .card.has-warnings {{ border-color: #78350f; }}
  .card-header {{
    display: flex; align-items: baseline; justify-content: space-between;
    padding: 0.75rem 1rem; background: var(--surface2);
    border-bottom: 1px solid var(--border);
  }}
  .card-name {{ font-weight: bold; font-size: 0.95rem; color: var(--cyan); }}
  .card-path {{ color: var(--muted); font-size: 0.72rem; }}
  .card-body {{ padding: 0.75rem 1rem; }}
  .card-section {{ margin-bottom: 0.75rem; }}
  .card-section:last-child {{ margin-bottom: 0; }}
  .card-label {{
    font-size: 0.68rem; text-transform: uppercase; letter-spacing: 0.12em;
    color: var(--muted); margin-bottom: 0.3rem;
  }}

  /* scenario accordion */
  .scenario {{ margin-bottom: 0.5rem; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }}
  .scenario-hdr {{
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.35rem 0.75rem; background: var(--surface2);
    cursor: pointer; user-select: none; font-size: 0.8rem;
  }}
  .scenario-hdr:hover {{ background: #262a3d; }}
  .scenario-label {{ color: var(--text); font-weight: 600; }}
  .scenario-body {{ padding: 0.5rem 0.75rem; display: none; }}
  .scenario-body.open {{ display: block; }}
  .subsection-title {{
    font-size: 0.65rem; text-transform: uppercase; letter-spacing: 0.1em;
    color: var(--muted); margin: 0.4rem 0 0.2rem;
  }}

  /* warnings */
  .warnings-box {{
    border: 1px solid #78350f; background: #1c1207;
    border-radius: 8px; padding: 1rem 1.25rem; margin-bottom: 2rem;
  }}
  .warning-item {{
    display: flex; gap: 0.75rem; padding: 0.35rem 0;
    border-bottom: 1px solid #2a1c0a; font-size: 0.8rem;
  }}
  .warning-item:last-child {{ border-bottom: none; }}
  .warn-fix {{ color: var(--yellow); font-weight: bold; min-width: 180px; }}
  .warn-msg {{ color: #fde68a; }}
  .all-ok {{
    border: 1px solid #14532d; background: #0a1f12; border-radius: 8px;
    padding: 0.75rem 1.25rem; margin-bottom: 2rem;
    color: var(--green); font-size: 0.85rem;
  }}

  .section {{ display: none; }}
  .section.active {{ display: block; }}

  /* layer legend */
  .legend {{
    display: flex; flex-wrap: wrap; gap: 0.4rem;
    margin-bottom: 1.5rem; font-size: 0.72rem;
  }}
  .legend-item {{ display: flex; align-items: center; gap: 0.3rem; }}
</style>
</head>
<body>

<header>
  <h1>env-flow — Layer Report</h1>
  <div class="meta">Generated {date} · {fixture_path} · {total} fixtures · {ok_count} ok · {warn_count} warning(s)</div>
  <div class="counters">
    <div class="counter ok">
      <div class="num">{ok_count}</div>
      <div class="label">fixtures ok</div>
    </div>
    <div class="counter warn">
      <div class="num">{warn_count}</div>
      <div class="label">warnings</div>
    </div>
    <div class="counter">
      <div class="num">{total}</div>
      <div class="label">total</div>
    </div>
  </div>
</header>

<nav>
  <div class="tab active" onclick="showSection('summary')">Summary Table</div>
  <div class="tab" onclick="showSection('cards')">Detail Cards</div>
  <div class="tab" onclick="showSection('mode-comparison')">Mode Comparison</div>
  <div class="tab" onclick="showSection('warnings-section')">Warnings {warn_badge}</div>
  <div class="tab" onclick="showSection('legend-section')">Layer Legend</div>
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

<div id="mode-comparison" class="section">
  <div class="section-title">Mode comparison — cascade vs no-cascade vs from-file</div>
  {mode_comparison_html}
</div>

<div id="warnings-section" class="section">
  <div class="section-title">Warnings &amp; anomalies</div>
  {warnings_html}
</div>

<div id="legend-section" class="section">
  <div class="section-title">Layer model</div>
  {legend_html}
</div>

<script>
function showSection(id) {{
  document.querySelectorAll('.section').forEach(s => s.classList.remove('active'));
  document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
  document.getElementById(id).classList.add('active');
  event.target.classList.add('active');
}}
function goToCard(id) {{
  document.querySelectorAll('.section').forEach(s => s.classList.remove('active'));
  document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
  document.getElementById('cards').classList.add('active');
  document.querySelectorAll('.tab')[1].classList.add('active');
  setTimeout(() => document.getElementById('card-' + id)?.scrollIntoView({{behavior:'smooth'}}), 50);
}}
function toggleScenario(el) {{
  el.nextElementSibling.classList.toggle('open');
}}
</script>
</body>
</html>"#,
        date = date,
        fixture_path = fixture_path,
        total = total,
        ok_count = ok_count,
        warn_count = warn_count,
        warn_badge = warn_badge,
        table_html = table_html,
        cards_html = cards_html,
        warnings_html = warnings_html,
        mode_comparison_html = mode_comparison_html,
        legend_html = render_legend(),
    )
}

// ─────────────────────────────────────────────────────────────
// Summary table
// ─────────────────────────────────────────────────────────────

fn render_summary_table(results: &[FixtureResult]) -> String {
    let rows: String = results.iter().map(|r| {
        let stage_badges: String = r.stages.iter()
            .map(|s| format!(r#"<span class="badge badge-stage">{s}</span>"#))
            .collect();
        let ctx_badges: String = r.contexts.iter()
            .map(|c| format!(r#"<span class="badge badge-ctx">{c}</span>"#))
            .collect();
        let file_badge = format!(r#"<span class="badge badge-file">{} files</span>"#, r.file_count);
        let status = if r.warnings.is_empty() {
            r#"<span class="status-ok">✓</span>"#.to_string()
        } else {
            let msgs = r.warnings.join("; ");
            format!(r#"<span class="status-warn" title="{msgs}">⚠ {}</span>"#, r.warnings.len())
        };

        format!(r#"<tr>
          <td class="fix-name"><a href="javascript:goToCard('{name}')">{name}</a></td>
          <td>{file_badge}</td>
          <td>{stage_badges}</td>
          <td>{ctx_badges}</td>
          <td>{status}</td>
        </tr>"#, name = r.name)
    }).collect();

    format!(r#"<table>
      <thead><tr>
        <th>Fixture</th>
        <th>Files</th>
        <th>Stages detected</th>
        <th>Contexts detected</th>
        <th>Status</th>
      </tr></thead>
      <tbody>{rows}</tbody>
    </table>"#)
}

// ─────────────────────────────────────────────────────────────
// Detail cards
// ─────────────────────────────────────────────────────────────

fn render_card(r: &FixtureResult) -> String {
    let has_warn = !r.warnings.is_empty();
    let warn_tag = if has_warn {
        format!(r#"<span class="badge badge-warn">⚠ {}</span>"#, r.warnings.len())
    } else {
        r#"<span class="status-ok">✓</span>"#.to_string()
    };

    let scenarios_html: String = r.scenarios.iter().enumerate().map(|(i, s)| {
        render_scenario(s, i == 0)
    }).collect();

    let no_scenarios = if r.scenarios.is_empty() {
        r#"<div style="color:var(--muted);font-size:0.8rem">No .env files found in this directory.</div>"#
    } else { "" };

    format!(r#"<div class="card{extra}" id="card-{name}">
      <div class="card-header">
        <div>
          <span class="card-name">{name}</span>
          <div class="card-path">{path}</div>
        </div>
        {warn_tag}
      </div>
      <div class="card-body">
        <div class="card-section">
          <div class="card-label">Scenarios ({count} combinations)</div>
          {scenarios_html}
          {no_scenarios}
        </div>
      </div>
    </div>"#,
        extra = if has_warn { " has-warnings" } else { "" },
        name = r.name,
        path = r.path.display(),
        count = r.scenarios.len(),
    )
}

fn render_scenario(s: &ScenarioResult, open: bool) -> String {
    let open_cls = if open { " open" } else { "" };

    let mode_badge = format!(
        r#"<span class="badge {}">{}</span>"#,
        s.mode.css_class(), s.mode.label()
    );

    let status_badge = match &s.vars {
        Ok(v) => format!(r#"<span class="badge badge-ok">{} vars</span>"#, v.len()),
        Err(e) => format!(r#"<span class="badge badge-err" title="{e}">✗ error</span>"#),
    };

    let layers_html = render_layers(&s.layers);
    let vars_html = match &s.vars {
        Ok(v) if !v.is_empty() => render_vars(v),
        Ok(_) => r#"<span style="color:var(--muted);font-size:0.75rem">— no variables —</span>"#.to_string(),
        Err(e) => format!(r#"<div style="color:var(--red);font-size:0.75rem">{e}</div>"#),
    };

    format!(r#"<div class="scenario">
      <div class="scenario-hdr" onclick="toggleScenario(this)">
        <span class="scenario-label">{label}</span>
        <span style="display:flex;gap:0.3rem;align-items:center">
          {mode_badge}
          {status_badge}
        </span>
      </div>
      <div class="scenario-body{open_cls}">
        <div class="subsection-title">Layer chain (low → high priority)</div>
        {layers_html}
        <div class="subsection-title" style="margin-top:0.6rem">Resolved variables</div>
        {vars_html}
      </div>
    </div>"#,
        label = s.label,
    )
}

fn render_layers(layers: &[ResolvedLayer]) -> String {
    if layers.is_empty() {
        return r#"<div style="color:var(--muted);font-size:0.75rem">— no layers —</div>"#.to_string();
    }
    let rows: String = layers.iter().map(|l| {
        let type_name = layer_type_name(&l.layer_type);
        let pill_cls = format!("lp-{}", type_name.replace(' ', ""));
        let path_cls = if l.exists { "layer-path" } else { "layer-path missing" };
        let tick = if l.exists {
            r#"<span class="layer-check">✓</span>"#
        } else {
            r#"<span class="layer-dash">○</span>"#
        };
        format!(r#"<div class="layer-row">
          {tick}
          <span class="layer-pill {pill_cls}">{type_name}</span>
          <span class="{path_cls}">{rel}</span>
        </div>"#,
            type_name = type_name,
            rel = l.relative_path,
        )
    }).collect();
    format!(r#"<div style="margin:0.2rem 0">{rows}</div>"#)
}

fn layer_type_name(lt: &LayerType) -> &'static str {
    match lt {
        LayerType::Base              => "Base",
        LayerType::ContextBase       => "ContextBase",
        LayerType::StageBase         => "StageBase",
        LayerType::StageContext      => "StageContext",
        LayerType::LocalOverride     => "LocalOverride",
        LayerType::StageLocalOverride => "StageLocalOverride",
    }
}

fn render_vars(vars: &EnvVars) -> String {
    let rows: String = vars.iter().map(|(k, v)| {
        let (_, path) = vars.get_with_source(k).unwrap_or((v, Path::new("")));
        let src = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        // Truncate long values
        let display_val = if v.len() > 80 {
            format!("{}…", &v[..80])
        } else {
            v.replace('<', "&lt;").replace('>', "&gt;")
        };
        format!(r#"<tr>
          <td class="var-key">{k}</td>
          <td class="var-val">{display_val}</td>
          <td class="var-src">{src}</td>
        </tr>"#)
    }).collect();

    format!(r#"<table class="vars-table">
      <thead><tr>
        <td style="color:var(--muted);font-size:0.65rem">KEY</td>
        <td style="color:var(--muted);font-size:0.65rem">VALUE</td>
        <td style="color:var(--muted);font-size:0.65rem">SOURCE</td>
      </tr></thead>
      <tbody>{rows}</tbody>
    </table>"#)
}

// ─────────────────────────────────────────────────────────────
// Warnings panel
// ─────────────────────────────────────────────────────────────

fn render_warnings(warnings: &[Warning]) -> String {
    if warnings.is_empty() {
        return r#"<div class="all-ok">✓ No warnings — all fixtures loaded correctly.</div>"#.to_string();
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
// Layer legend
// ─────────────────────────────────────────────────────────────

fn render_mode_comparison(results: &[FixtureResult]) -> String {
    // Find fixtures that have at least one cascade + one no-cascade scenario
    let comparable: Vec<&FixtureResult> = results
        .iter()
        .filter(|r| {
            let has_cascade    = r.scenarios.iter().any(|s| s.mode == LoadMode::Cascade);
            let has_no_cascade = r.scenarios.iter().any(|s| s.mode == LoadMode::NoCascade);
            has_cascade && has_no_cascade
        })
        .collect();

    if comparable.is_empty() {
        return r#"<div class="all-ok">No fixtures with multiple layers found.</div>"#.to_string();
    }

    let explanation = r#"<div style="color:var(--muted);font-size:0.8rem;margin-bottom:1.5rem;line-height:1.8">
      <strong style="color:var(--text)">cascade</strong> (default) — all layers merged; lower layers provide fallback values.<br>
      <strong style="color:var(--text)">no-cascade</strong> — only the single highest-priority <em>existing</em> file is loaded; no inheritance.<br>
      <strong style="color:var(--text)">from-file</strong> — one specific file loaded directly; the layer chain is ignored entirely.
    </div>"#;

    let tables: String = comparable.iter().map(|r| {
        // For each unique base-label, collect cascade vs no-cascade scenarios
        let base_labels: Vec<String> = r.scenarios.iter()
            .filter(|s| s.mode == LoadMode::Cascade)
            .map(|s| s.label.clone())
            .collect();

        let rows: String = base_labels.iter().map(|base| {
            let cascade = r.scenarios.iter()
                .find(|s| s.mode == LoadMode::Cascade && &s.label == base);
            let no_cascade = r.scenarios.iter()
                .find(|s| s.mode == LoadMode::NoCascade && s.label.starts_with(base.as_str()));

            let fmt_vars = |vars: &Result<EnvVars, env_flow::Error>| -> String {
                match vars {
                    Err(e) => format!(r#"<span style="color:var(--red)">{e}</span>"#),
                    Ok(v) if v.is_empty() => r#"<span style="color:var(--muted)">—</span>"#.to_string(),
                    Ok(v) => {
                        let pairs: String = v.iter()
                            .map(|(k, val)| format!(
                                r#"<div><span style="color:var(--cyan)">{k}</span>=<span style="color:var(--text)">{val}</span></div>"#
                            ))
                            .collect();
                        format!(r#"<div style="font-size:0.72rem;line-height:1.7">{pairs}</div>"#)
                    }
                }
            };

            let cascade_cell = cascade
                .map(|s| fmt_vars(&s.vars))
                .unwrap_or_else(|| r#"<span style="color:var(--muted)">—</span>"#.to_string());
            let nc_cell = no_cascade
                .map(|s| fmt_vars(&s.vars))
                .unwrap_or_else(|| r#"<span style="color:var(--muted)">—</span>"#.to_string());

            // Count vars for badge
            let cascade_count = cascade.and_then(|s| s.vars.as_ref().ok()).map(|v| v.len()).unwrap_or(0);
            let nc_count = no_cascade.and_then(|s| s.vars.as_ref().ok()).map(|v| v.len()).unwrap_or(0);
            let saved_label = if cascade_count > nc_count {
                format!(r#"<span style="color:var(--muted);font-size:0.68rem">{} fewer vars</span>"#,
                    cascade_count - nc_count)
            } else { String::new() };

            format!(r#"<tr>
              <td style="padding:0.5rem 0.6rem;color:var(--text);font-weight:600;white-space:nowrap">{base}</td>
              <td style="padding:0.5rem 0.6rem;border-left:1px solid var(--border)">{cascade_cell}</td>
              <td style="padding:0.5rem 0.6rem;border-left:1px solid var(--border)">{nc_cell} {saved_label}</td>
            </tr>"#)
        }).collect();

        format!(r#"<div style="margin-bottom:2rem">
          <div style="font-size:0.8rem;font-weight:bold;color:var(--cyan);margin-bottom:0.5rem">{name}</div>
          <table style="width:100%;border-collapse:collapse;font-size:0.78rem">
            <thead><tr>
              <th style="padding:0.3rem 0.6rem;color:var(--muted);text-align:left">Scenario</th>
              <th style="padding:0.3rem 0.6rem;color:var(--blue);text-align:left;border-left:1px solid var(--border)">
                <span class="badge badge-mode-cascade">cascade</span> (all layers merged)
              </th>
              <th style="padding:0.3rem 0.6rem;color:var(--orange);text-align:left;border-left:1px solid var(--border)">
                <span class="badge badge-mode-nocascade">no-cascade</span> (highest layer only)
              </th>
            </tr></thead>
            <tbody>{rows}</tbody>
          </table>
        </div>"#, name = r.name)
    }).collect();

    format!("{explanation}{tables}")
}

// ─────────────────────────────────────────────────────────────

fn render_legend() -> String {
    let layers = [
        ("Base",              "lp-Base",              "`.env` — always loaded first"),
        ("ContextBase",       "lp-ContextBase",       "`.env.{ctx}` or `{ctx}/.env` — skipped for Local context"),
        ("StageBase",        "lp-StageBase",         "`.env.{stage}` — stage-specific overrides"),
        ("StageContext",     "lp-StageContext",      "`.env.{stage}.{ctx}` or `{ctx}/.env.{stage}`"),
        ("LocalOverride",    "lp-LocalOverride",     "`.env.local` — machine-local secrets; skipped in containers/CI"),
        ("StageLocalOverride","lp-StageLocalOverride","`.env.{stage}.local` — local + stage override; skipped in containers/CI"),
    ];

    let rows: String = layers.iter().map(|(name, cls, desc)| {
        format!(r#"<tr>
          <td style="padding:0.4rem 0.6rem"><span class="layer-pill {cls}">{name}</span></td>
          <td style="padding:0.4rem 0.6rem;color:var(--text);font-size:0.8rem">{desc}</td>
        </tr>"#)
    }).collect();

    let matrix = r#"
<div style="margin-top:1.5rem;margin-bottom:0.5rem;color:var(--muted);font-size:0.7rem;text-transform:uppercase;letter-spacing:0.1em">
  Context behaviour
</div>
<table style="width:auto;font-size:0.75rem">
  <thead><tr>
    <th style="padding:0.3rem 0.6rem">Context</th>
    <th style="padding:0.3rem 0.6rem">ContextBase loaded</th>
    <th style="padding:0.3rem 0.6rem">.local files loaded</th>
    <th style="padding:0.3rem 0.6rem">is_container()</th>
  </tr></thead>
  <tbody>
    <tr><td style="padding:0.3rem 0.6rem;color:var(--cyan)">Local</td>
        <td style="padding:0.3rem 0.6rem;color:var(--red)">✗ skipped</td>
        <td style="padding:0.3rem 0.6rem;color:var(--green)">✓ loaded</td>
        <td style="padding:0.3rem 0.6rem;color:var(--red)">✗</td></tr>
    <tr><td style="padding:0.3rem 0.6rem;color:var(--cyan)">Docker / DockerCompose</td>
        <td style="padding:0.3rem 0.6rem;color:var(--green)">✓ loaded</td>
        <td style="padding:0.3rem 0.6rem;color:var(--red)">✗ skipped</td>
        <td style="padding:0.3rem 0.6rem;color:var(--green)">✓</td></tr>
    <tr><td style="padding:0.3rem 0.6rem;color:var(--cyan)">Kubernetes</td>
        <td style="padding:0.3rem 0.6rem;color:var(--green)">✓ loaded</td>
        <td style="padding:0.3rem 0.6rem;color:var(--red)">✗ skipped</td>
        <td style="padding:0.3rem 0.6rem;color:var(--green)">✓</td></tr>
    <tr><td style="padding:0.3rem 0.6rem;color:var(--cyan)">OrbStack</td>
        <td style="padding:0.3rem 0.6rem;color:var(--green)">✓ loaded</td>
        <td style="padding:0.3rem 0.6rem;color:var(--red)">✗ skipped</td>
        <td style="padding:0.3rem 0.6rem;color:var(--green)">✓</td></tr>
    <tr><td style="padding:0.3rem 0.6rem;color:var(--cyan)">CI</td>
        <td style="padding:0.3rem 0.6rem;color:var(--green)">✓ loaded</td>
        <td style="padding:0.3rem 0.6rem;color:var(--red)">✗ skipped</td>
        <td style="padding:0.3rem 0.6rem;color:var(--red)">✗</td></tr>
  </tbody>
</table>"#;

    format!(r#"
<div style="margin-bottom:0.5rem;color:var(--muted);font-size:0.7rem;text-transform:uppercase;letter-spacing:0.1em">
  Layer types (low → high priority)
</div>
<table style="width:auto;font-size:0.75rem">
  <thead><tr>
    <th style="padding:0.3rem 0.6rem">Layer</th>
    <th style="padding:0.3rem 0.6rem">Description</th>
  </tr></thead>
  <tbody>{rows}</tbody>
</table>
{matrix}
"#)
}

// ─────────────────────────────────────────────────────────────
// Utilities
// ─────────────────────────────────────────────────────────────

fn now_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let s = secs % 86400;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    let days = secs / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{sec:02} UTC")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut y = 1970u64;
    loop {
        let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
        let dy = if leap { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    let months = [31u64, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u64;
    for dm in months {
        if days < dm { break; }
        days -= dm;
        mo += 1;
    }
    (y, mo, days + 1)
}
