# env-flow Integration

**Status:** Complete — devcli-core uses `env-flow` for all env loading and file discovery.

---

## Architecture

```
devcli-core
├── env_flow_support.rs     ← runtime: cascade, container merge, config pins, layers
├── detection/env_files.rs  ← auto-add: discover files + build config maps (env-flow detector)
└── commands/prepare.rs     ← start/run consumption
```

No legacy loaders.

---

## Runtime (`env_flow_support`)

| API | Use |
|-----|-----|
| `load_local_runtime` | Local cascade; config pin only when stage/context path is set |
| `load_process_runtime` | Local / k8s / ci process env injection |
| `prepare_container_env_file` | Docker / OrbStack / Compose `--env-file` (merged temp file when needed) |
| `resolve_runtime_env_display` | `config list` / `config show` with layer list |
| `resolve_runtime_layers` | Layer introspection |
| `effective_stage` | Config stage or auto-detect (`APP_ENV`, `NODE_ENV`, …) |

Empty values → `"XXX"`. `${VAR}` interpolation on.

**Config map:** `env_files` pins a path only for the matching stage + context. Missing entries fall back to cascade (auto-add no longer disables local cascade).

**Reverse patterns:** `.local.env`, `.dev.env`, `.dev.local.env` supported via env-flow detector + resolver.

**Environments:** `local`, `docker`, `orbstack`, `docker-compose`, `k8s`, `ci`.

---

## Detection (`env_files`)

`detect_env_files` uses `env_flow::detector::discover`. Builds config maps via `build_env_files_map*`.

---

## Tests

```bash
cargo test -p env-flow
cargo test -p devcli-core
```
