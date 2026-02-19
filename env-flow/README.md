# env-flow

A **2D layered `.env` loader** for Rust.

Most `.env` crates load one file. `env-flow` loads the right *stack* of files
for a given **stage** (dev / qa / prod / …) × **runtime context**
(local machine / Docker / Kubernetes / CI), merges them in priority order,
and gives you source tracking and `${VAR}` interpolation on top.

---

## The layer model

```
Layer 1 (lowest):  .env
Layer 2:           .env.{context}  OR  {context}/.env        ← skipped for Local
Layer 3:           .env.{stage}
Layer 4:           .env.{stage}.{context}  OR  {context}/.env.{stage}   ← skipped for Local
Layer 5:           .env.local                                ← skipped in Docker/K8s/CI
Layer 6 (highest): .env.{stage}.local                        ← skipped in Docker/K8s/CI
```

Higher layers override lower ones. Missing files are silently skipped.
`.local` files are machine-specific secrets — containers and CI skip them because
there is no "local machine" in those environments.

Both **suffix-style** (`.env.docker`) and **directory-style** (`docker/.env`)
naming conventions are supported. When both exist for the same logical layer,
suffix-style wins.

---

## Quick start

```toml
[dependencies]
env-flow = "0.1"
```

```rust
use env_flow::{EnvFlow, RuntimeContext, Stage};

// Explicit stage + context
let vars = EnvFlow::from_dir("./my-app")
    .stage(Stage::Dev)
    .context(RuntimeContext::Docker)
    .load()?;

println!("PORT = {}", vars.get("PORT").unwrap_or("3000"));

// Auto-detect everything from the process environment
let vars = EnvFlow::from_dir(".").auto_detect().load()?;
vars.apply(); // push all vars into std::env
```

---

## Builder API

### Constructors

| Constructor | Description |
|---|---|
| `EnvFlow::from_dir(path)` | Load via the full layer chain |
| `EnvFlow::from_file(path)` | Load exactly one file, no layering, no inheritance |

### Builder methods

| Method | Default | Description |
|---|---|---|
| `.stage(Stage::Dev)` | none | Set deployment stage |
| `.stage_opt(Option<Stage>)` | — | Set stage from an `Option` |
| `.context(RuntimeContext::Docker)` | `Local` | Set runtime context |
| `.interpolate(bool)` | `true` | Enable/disable `${VAR}` expansion |
| `.strict(bool)` | `false` | Malformed lines → error instead of warning |
| `.auto_detect()` | — | Infer stage and context from the process environment |
| `.no_cascade()` | off | Only load the single highest-priority existing file |

### Terminal methods

| Method | Returns | Description |
|---|---|---|
| `.load()` | `Result<EnvVars>` | Load, merge, and interpolate |
| `.layers()` | `Result<Vec<ResolvedLayer>>` | Inspect the layer plan without loading |

---

## Loading modes

### `from_dir` — cascade (default)

All layers merged from lowest to highest priority. A key set in `.env` and
overridden in `.env.dev` ends up with the `.env.dev` value; keys only in `.env`
are still available.

```rust
let vars = EnvFlow::from_dir(".").stage(Stage::Dev).load()?;
vars.get("PORT");     // from .env if not overridden
vars.get("DB_HOST");  // from .env.dev if set there
```

### `from_dir` + `.no_cascade()` — isolated

The resolver computes the full chain, finds the highest-priority file that
exists on disk, and loads **only that file**. Keys in lower-priority files are
not inherited.

```rust
// .env=PORT=3000  .env.dev=DB=dev-db  .env.dev.local=SECRET=x
let vars = EnvFlow::from_dir(".")
    .stage(Stage::Dev)
    .context(RuntimeContext::Local)
    .no_cascade()
    .load()?;

vars.get("SECRET"); // "x"   — from .env.dev.local (highest)
vars.get("PORT");   // None  — .env was not loaded
vars.get("DB");     // None  — .env.dev was not loaded
```

`.layers()` still returns the full chain for transparency even in no-cascade
mode — you can see what exists but only the winner is loaded.

### `from_file` — single file

Bypasses the layer chain completely. Useful when you know exactly which file
you want.

```rust
let vars = EnvFlow::from_file(".env.prod").load()?;
vars.get("PORT");     // only what's in .env.prod
```

### Choosing a mode

| Need | Mode |
|---|---|
| Stage overrides with fallback to base | `from_dir` (cascade, default) |
| Strict isolation — only one file applies | `from_dir` + `.no_cascade()` |
| Load a specific known file | `from_file` |

---

## Stages

```rust
use env_flow::Stage;

Stage::Dev      // "dev" or "development"
Stage::Qa       // "qa"
Stage::Staging  // "staging"
Stage::Preprod  // "preprod", "pre-prod", "pre_prod"
Stage::Prod     // "prod" or "production"
Stage::Test     // "test"
Stage::Custom("nightly".into())

// From string (e.g. read from APP_ENV)
let stage = Stage::from("dev");   // Stage::Dev
let stage = Stage::from("beta");  // Stage::Custom("beta")
```

---

## Runtime contexts

```rust
use env_flow::RuntimeContext;

RuntimeContext::Local          // developer machine (default)
RuntimeContext::Docker         // docker run / docker build
RuntimeContext::DockerCompose  // COMPOSE_PROJECT_NAME is set
RuntimeContext::Kubernetes     // KUBERNETES_SERVICE_HOST is set
RuntimeContext::OrbStack       // macOS container runtime
RuntimeContext::CI             // GitHub Actions, GitLab CI, CircleCI, Jenkins, …
RuntimeContext::Custom("my-env".into())

ctx.skip_local_files() // true for Docker/K8s/OrbStack/CI
ctx.is_container()     // true for Docker/DockerCompose/K8s/OrbStack
```

Both `Docker` and `DockerCompose` use `"docker"` as the file-name key
(`.env.docker` / `docker/.env`).

---

## Auto-detection

```rust
let vars = EnvFlow::from_dir(".").auto_detect().load()?;
```

**Stage** — first match wins:
`APP_ENV` → `RUST_ENV` → `NODE_ENV` → `RAILS_ENV` → `FLASK_ENV`

**Runtime context** — checked in order:
1. `KUBERNETES_SERVICE_HOST` set → Kubernetes
2. `/.dockerenv` exists + OrbStack marker → OrbStack
3. `/.dockerenv` exists → Docker
4. `CI=true/1` or `GITHUB_ACTIONS` / `GITLAB_CI` / `CIRCLECI` / `JENKINS_URL` → CI
5. `COMPOSE_PROJECT_NAME` set → DockerCompose
6. Otherwise → Local

---

## Working with `EnvVars`

```rust
let vars = EnvFlow::from_dir(".").load()?;

vars.get("PORT")                    // Option<&str>
vars.get_parsed::<u16>("PORT")?     // Result<u16>
vars.get_with_source("DB_URL")      // Option<(&str, &Path)>  — value + source file
vars.require(&["DATABASE_URL"])?    // Err if any key is missing
vars.apply()                        // push all into std::env
vars.iter()                         // (key, value) in insertion order
vars.len()
vars.is_empty()
```

### Inspecting the layer plan

```rust
let plan = EnvFlow::from_dir(".")
    .stage(Stage::Dev)
    .context(RuntimeContext::Local)
    .layers()?;

for layer in &plan {
    println!("{:?}  {}  exists={}",
        layer.layer_type, layer.relative_path, layer.exists);
}
```

---

## Interpolation

`${VAR}` and `${VAR:-default}` are expanded after all layers are merged:

```sh
# .env
BASE_URL=https://api.example.com
FULL_URL=${BASE_URL}/v2

# .env.dev
BASE_URL=https://dev.example.com
```

```rust
// Stage dev: BASE_URL="https://dev.example.com", FULL_URL="https://dev.example.com/v2"
let vars = EnvFlow::from_dir(".").stage(Stage::Dev).load()?;
```

| Syntax | Behaviour |
|---|---|
| `${VAR}` | Substitute value of VAR (env map first, then `std::env`) |
| `${VAR:-default}` | Substitute VAR, or "default" if unset |
| `\${VAR}` | Literal `${VAR}` — escaped |
| `'single quoted'` | Never interpolated (bash semantics) |

Circular references (`A=${B}`, `B=${A}`) return `Error::CircularInterpolation`.

Disable interpolation:
```rust
EnvFlow::from_dir(".").interpolate(false).load()?;
```

---

## Parser details

| Input | Result |
|---|---|
| `KEY=value` | `("KEY", "value")` |
| `export KEY=value` | `("KEY", "value")` |
| `KEY=` | `("KEY", "")` — empty string, not a placeholder |
| `KEY="hello world"` | `("KEY", "hello world")` |
| `KEY='no ${interp}'` | `("KEY", "no ${interp}")` — no interpolation |
| `KEY="line1\nline2"` | `("KEY", "line1↵line2")` — escape sequences |
| `URL=postgres://h/db?a=1` | `("URL", "postgres://h/db?a=1")` — split on first `=` |
| `KEY=value # comment` | `("KEY", "value")` — space before `#` = comment |
| `URL=https://x.com#frag` | `("URL", "https://x.com#frag")` — no space = not comment |

Also handles: UTF-8 BOM, CRLF line endings, multiline double-quoted values, Unicode.

---

## HTML report

Visualise all `.env` fixtures in a browser, showing layer plans, resolved
variables, mode comparisons, and warnings:

```sh
cargo run --example report -p env-flow

# or point at your own project
cargo run --example report -p env-flow -- /path/to/your/app
```

---

## Examples

```sh
cargo run --example basic          -p env-flow   # explicit stage + context
cargo run --example auto_detect    -p env-flow   # auto-detect from environment
cargo run --example apply_to_process -p env-flow # push vars into std::env
cargo run --example report         -p env-flow   # HTML report of fixtures
```
