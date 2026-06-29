# env-flow Integration Plan

**Date:** 2026-06-29  
**Branch:** `use-env-flow`  
**Status:** In progress — Phase 0–1 complete  
**Reference:** [workspace-crate-audit.md](../reports/workspace-crate-audit.md)

---

## Executive Summary

`env-flow` is a production-ready 6-layer `.env` loader (132 tests). `devcli-core` still uses inline logic in `detection/environments/env_files.rs` and `commands/prepare.rs`. Only `process-manager` completed the full extract → integrate path.

**Goal:** Wire `env-flow` as the runtime env loader/parser in `devcli-core`, while keeping devcli-specific concerns (config `env_files` map, auto-add detection, interactive mapping, TUI) in the product crate.

---

## Current Architecture

```
devcli-core dependencies today:
  ✅ process-manager
  ❌ env-flow      ← this integration
  ❌ app-detector
  ❌ config-manager
```

### Duplication Map

| Concern | Lives in devcli-core | Extracted to | Integrated? |
|---------|---------------------|--------------|-------------|
| Process lifecycle | adapter only | process-manager | ✅ |
| Env file loading | `env_files.rs` + `prepare.rs` | env-flow | ❌ → in progress |
| App detection | `detection/` | app-detector | ❌ |
| Config load/resolve | `config/` | config-manager | ❌ |

### Runtime Path Today

1. `resolve_env_file_path` — config map (strict) or legacy `find_env_file` (picks **one** file)
2. **Docker/OrbStack** — inject `--env-file` with that single path
3. **Local** — `parse_env_file` on that single file (naive parser, no cascade)

### env-flow Layer Model

```
Layer 1 (lowest):  .env
Layer 2:           .env.{context}  OR  {context}/.env        ← skipped for Local
Layer 3:           .env.{stage}
Layer 4:           .env.{stage}.{context}  OR  {context}/.env.{stage}  ← skipped for Local
Layer 5:           .env.local                                ← skipped in Docker/K8s/CI
Layer 6 (highest): .env.{stage}.local                        ← skipped in containers/CI
```

### Feature Parity Matrix

| Capability | env-flow | devcli-core (current) |
|------------|:--------:|:---------------------:|
| Multi-layer cascade merge | ✅ | ❌ single file only |
| `${VAR}` interpolation | ✅ | ❌ |
| Robust parser (multiline, export, escapes) | ✅ | ❌ naive line-split |
| Source tracking / override history | ✅ | ❌ |
| `layers()` introspection | ✅ | ❌ |
| `auto_detect` stage/context | ✅ | ❌ |
| Config `env_files` map (strict) | ❌ | ✅ |
| Interactive env assignment (auto_add) | ❌ | ✅ |
| Docker ↔ OrbStack aliasing | ❌ | ✅ |
| Dockerfile-nested paths | ❌ | ✅ |
| `.*.env` reverse pattern (`.local.env`) | ❌ | ✅ |
| Empty value → `"XXX"` placeholder | ❌ | ✅ |
| Docker `--env-file` injection | ❌ | ✅ (in prepare.rs) |
| Typed `Stage` enum | ✅ | ❌ (uses `&str`) |
| `DockerCompose` / `CI` contexts | ✅ | ❌ |

---

## Target Architecture

Modeled on `process-manager` integration (`process_manager_support.rs`):

| Layer | Responsibility |
|-------|----------------|
| **Crate** (`env-flow`) | Generic layer resolution, parsing, merge, interpolation |
| **Adapter** (`env_flow_support.rs`) | devcli types (`&str` stage), paths, config map bridge, behavioral quirks |
| **Product** (`prepare.rs`, commands) | Orchestration only — calls adapter, not env-flow directly |

```
devcli-core
├── env_flow_support.rs     ← adapter (like process_manager_support.rs)
├── detection/environments/
│   └── env_files.rs        ← detection + config map; runtime loading delegated
└── commands/prepare.rs     ← calls adapter for load
```

### Key Files

| Purpose | Path |
|---------|------|
| env-flow entry + API | `env-flow/src/lib.rs` |
| Layer resolver | `env-flow/src/resolver.rs` |
| devcli env logic | `devcli-core/src/detection/environments/env_files.rs` |
| Runtime consumption | `devcli-core/src/commands/prepare.rs` |
| Integration tests | `env-flow/tests/integration_tests.rs` |
| Adapter | `devcli-core/src/env_flow_support.rs` |

---

## Decisions (locked)

### 1. Cascade vs single-file

| Runtime | Today | Target |
|---------|-------|--------|
| **Local** | One file via priority | **Cascade** (Phase 2) — main product win |
| **Docker/OrbStack** | One file via `--env-file` | **Phase 3a:** keep single-file; **Phase 3b (optional):** temp merged file |

### 2. Loading root for Docker context

env-flow resolves from one `root`. devcli searches both app root and dockerfile parent.

**Approach:** adapter tries app root first; for Docker/OrbStack with `dockerfile_path`, also consider `dockerfile_parent` (Phase 5).

### 3. OrbStack context

Map `"orbstack"` → `RuntimeContext::OrbStack` (not `Docker`).

### 4. Empty values

Post-process in adapter: `""` → `"XXX"` when converting `EnvVars` → `HashMap<String, String>`.

### 5. `.*.env` reverse pattern

Keep in **detection only** (`detect_env_files` for auto-add/TUI). Does not block runtime integration.

---

## Phased Plan

### Phase 0 — Prep ✅

- [x] Add `env-flow` to `devcli-core/Cargo.toml`
- [x] Create `devcli-core/src/env_flow_support.rs`
- [x] Add `mod env_flow_support` in `lib.rs`
- [x] Baseline: `cargo test -p devcli-core` + `cargo test -p env-flow` green

### Phase 1 — Parser swap ✅

**Target:** `parse_env_file` in `env_files.rs`

Reimplement as thin wrapper around `EnvFlow::from_file(path).load()` + empty→`XXX` mapping.

**Done:**
- [x] `parse_env_file` delegates to `env_flow_support::parse_env_file`
- [x] Adapter tests: multiline, export, interpolation, empty→`XXX`, cascade helper
- [x] `orbstack_stage_test.rs` still passes (9 tests)

### Phase 2 — Local cascade in `prepare.rs` (highest value)

**Target:** local branch in `prepare.rs`

When `environment == "local"` and no strict config map hit:
- `env_flow_support::load_cascade(working_dir, stage, Local)` instead of single-file path
- Strict config map → `EnvFlow::from_file(configured_path)` only

### Phase 3 — Docker / OrbStack

**3a (safe):** single-file via `from_file` or `no_cascade()`  
**3b (optional):** cascade → temp merged `.env` → `--env-file`

### Phase 4 — Consolidate runtime loaders

Delegate `load_env_vars_for_runtime`, `resolve_env_file_path`, deprecate or wrap `find_env_file`.

**Call sites:** `prepare.rs`, `config/list.rs`, `orbstack_stage_test.rs`

**Keep unchanged:** `detect_env_files`, `build_env_files_map*`, `auto_add/*`, `commands/env.rs`, TUI

### Phase 5 — Dockerfile-nested root gap

Adapter: when `dockerfile_path` set and context is Docker/OrbStack, run env-flow from `dockerfile_parent` when that dir has env files.

### Phase 6 — Cleanup

- Remove duplicate parser/priority logic from `env_files.rs`
- Add `env-flow/MIGRATION.md`
- Fix stale docs (`docs/devcli-core-command-lifecycle.md`, etc.)

---

## Test Strategy

| Layer | What to add |
|-------|-------------|
| **env-flow** | 132 tests unchanged; optional nested docker fixture |
| **env_flow_support** | Type mapping, empty→XXX, config strict, cascade, single-file docker |
| **devcli-core** | `prepare.rs` integration tests; update `orbstack_stage_test.rs` |
| **Regression** | `cargo test --workspace` |

---

## Risks

| Risk | Mitigation |
|------|------------|
| Local cascade changes effective env | Document; `.env.local` overrides become active (usually intended) |
| Interpolation surprises (`${VAR}`) | Adapter tests; `interpolate(false)` escape hatch |
| Strict config map + missing key | Preserve exact current behavior |
| Docker single-file vs cascade | Ship 3a first |
| `config list` display inaccurate post-cascade | Update to show layer plan |

---

## PR Breakdown

1. **PR 1 — Scaffold:** dependency + `env_flow_support` + parser swap (Phase 0–1)
2. **PR 2 — Local cascade:** `prepare.rs` local path (Phase 2)
3. **PR 3 — Docker/OrbStack:** single-file via env-flow (Phase 3a)
4. **PR 4 — Loader consolidation + nested dockerfile root (Phase 4–5)
5. **PR 5 — Cleanup + docs + migration guide (Phase 6)

---

## Definition of Done

- [ ] `env-flow` in `devcli-core/Cargo.toml`; no duplicate parser in `env_files.rs`
- [ ] Local runs use cascade; config map remains strict
- [ ] Docker/OrbStack behavior documented (single-file minimum)
- [ ] `cargo test -p devcli-core` and `cargo test -p env-flow` green
- [ ] `prepare.rs` has integration tests
- [ ] `env-flow/MIGRATION.md` describes behavioral deltas
- [ ] Detection/auto-add/TUI paths work without env-flow internals

---

## Related Documentation

| Document | Relevance |
|----------|-----------|
| [workspace-crate-audit.md](../reports/workspace-crate-audit.md) | Full audit |
| [env-flow/README.md](../env-flow/README.md) | Layer model and API |
| [process-manager/MIGRATION.md](../process-manager/MIGRATION.md) | Integration pattern reference |
| [devcli-core-command-lifecycle.md](./devcli-core-command-lifecycle.md) | Command flow (needs update post-integration) |
