# env-flow Integration

**Status:** Complete — devcli-core uses `env-flow` for all env loading and file discovery.

---

## Architecture

```
devcli-core
├── env_flow_support.rs     ← runtime: cascade, container paths, config-map loads
├── detection/env_files.rs  ← auto-add: discover files + build config maps (env-flow detector)
└── commands/prepare.rs     ← start/run consumption
```

No legacy loaders. No re-exports of old `find_env_file` / `parse_env_file` APIs from `detection`.

---

## Runtime (`env_flow_support`)

| API | Use |
|-----|-----|
| `load_local_runtime` | Local cascade or strict config-map file |
| `resolve_container_env_file_path` | Docker / OrbStack / k8s `--env-file` |
| `resolve_runtime_env_display` | `config list` / `config show` |

Empty values → `"XXX"`. `${VAR}` interpolation on.

---

## Detection (`env_files`)

`detect_env_files` uses `env_flow::detector::discover`. Builds config maps via `build_env_files_map*`.

---

## Tests

```bash
cargo test -p env-flow
cargo test -p devcli-core
```
