# env-flow Integration

**Date:** 2026-06-29  
**Branch:** `use-env-flow`  
**Status:** ✅ Complete

---

## Summary

`env-flow` is wired into `devcli-core` for all runtime env loading. Detection and config-map building stay in `detection/environments/env_files.rs`; loading goes through `env_flow_support.rs`.

```
devcli-core
├── env_flow_support.rs          ← runtime env loading (env-flow adapter)
├── detection/environments/
│   └── env_files.rs             ← detect + build config maps only
└── commands/prepare.rs          ← start/run env injection
```

---

## Runtime behavior

| Runtime | Loading |
|---------|---------|
| **Local** | Cascade: `.env` → stage → `.env.local` (via env-flow) |
| **Local** (config map) | Strict single file from `env_files` config |
| **Docker / OrbStack / k8s** | Highest env-flow layer for context, dockerfile-parent scan, `--env-file` |
| **Docker / OrbStack** (config map) | Strict single file from config |

**Devcli quirks** (in adapter): empty values → `"XXX"`; `${VAR}` interpolation enabled.

---

## Public API

| Function | Module | Purpose |
|----------|--------|---------|
| `parse_env_file` | `env_flow_support` | Single-file parse |
| `load_local_runtime` | `env_flow_support` | Local cascade or strict map |
| `resolve_container_env_file_path` | `env_flow_support` | Docker/OrbStack `--env-file` path |
| `resolve_runtime_env_display` | `env_flow_support` | `config list` display |
| `find_env_file` | `env_flow_support` | Legacy dockerfile-priority lookup |
| `detect_env_files` | `env_files` | Auto-add / TUI detection |
| `build_env_files_map*` | `env_files` | Config map building |

Re-exported from `devcli_core::detection` for backward compatibility.

---

## Key files

| Path | Role |
|------|------|
| `env-flow/src/lib.rs` | Layer model + `EnvFlow` builder |
| `devcli-core/src/env_flow_support.rs` | Adapter |
| `devcli-core/src/commands/prepare.rs` | Runtime consumption |
| `devcli-core/src/detection/environments/env_files.rs` | Detection only |
| `docs/devcli-core-command-lifecycle.md` | Command flow (§4.4 prepare) |

---

## Tests

```bash
cargo test -p env-flow
cargo test -p devcli-core
```

Coverage: adapter unit tests, `prepare.rs` integration tests, `orbstack_stage_test.rs` (legacy priority paths).

---

## Related

- [env-flow/README.md](../env-flow/README.md) — layer model reference
- [workspace-crate-audit.md](../reports/workspace-crate-audit.md) — pre-integration audit (2026-06-28)
