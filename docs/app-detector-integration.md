# app-detector Integration

**Status:** Complete — all detection orchestration goes through `app-detector`.

---

## Architecture

```
devcli-core
├── app_detector_support.rs   ← engine, report → DetectedApp, devcli command formatting
├── detection/mod.rs          ← public API (DetectedApp, detect_app, detect_app_type)
├── detection/nx.rs           ← Nx monorepo: hierarchical children + nx show project
└── detection/env_files.rs    ← env-flow file discovery (separate concern)
```

**Removed:** legacy `detection/environments/{local,docker,orbstack,k8s}.rs`, `app_types.rs`, `utils.rs`, `dockerfile.rs`.

---

## What app-detector owns

| Concern | Source |
|---------|--------|
| Two-phase detection orchestration | `DetectionEngine` |
| App type strategies (nodejs, nx, python, rust, redis, traefik) | app-detector |
| Local commands (`local-env` strategy) | app-detector |
| Docker / OrbStack / K8s capability detection | app-detector strategies |
| Hierarchical Nx workspace discovery (Phase 1.5) | app-detector |
| Nx workspace targets (`nx-env` strategy) | app-detector |

---

## What the adapter adds (devcli product layer)

| Concern | Location |
|---------|----------|
| App type priority + docker-compose redis/traefik heuristics | `resolve_app_type` |
| Docker port mappings + multi-stage stage keys | `build_docker_commands` |
| OrbStack `--context orbstack` commands | `build_orbstack_commands` |
| K8s `apply`/`delete`/`restart` path normalization | `normalize_k8s_commands` |
| `nx show project --json` per-app targets | `detection/nx.rs` |
| Workspace-level Nx commands (`run-many`, `affected`, …) | `detection/nx.rs` |
| Env file discovery + interactive mapping | `detection/env_files.rs` (env-flow) |
| Per-config Redis/Traefik splitting | `auto_add/single_app.rs` |

Environment commands are **gated** on app-detector strategy results (`docker`, `orbstack-env`, `kubernetes-env`).

---

## Adapter API

| Function | Use |
|----------|-----|
| `detect(path)` | Full detection with workspace recursion |
| `detect_single(path)` | Single directory (no Phase 1.5) |
| `detect_app(path)` | → `DetectedApp` |
| `build_detected_app(path, report)` | Report → `DetectedApp` |
| `detect_app_type` / `extract_app_name` | Phase 1 helpers |
| `workspace_child_paths(report)` | Nx monorepo children |

---

## App type priority

1. `nx` → 2. `nodejs` → 3. `redis` → 4. `traefik` → 5. docker-compose heuristics → 6. `python` → 7. `rust`

---

## Tests

```bash
cargo test -p app-detector
cargo test -p devcli-core
```

Adapter integration tests in `app_detector_support.rs` (11 tests). All legacy detection tests retained under `detection/tests/`.
