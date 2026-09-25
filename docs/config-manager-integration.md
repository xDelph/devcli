# config-manager Integration

**Status:** Complete — load/save, resolve, dependencies, and validate go through config-manager.

---

## Architecture

```
devcli-core
├── config_manager_support.rs  ← ConfigManager, Resolver, DependencyGraph, Validator
├── config/loader.rs           ← path discovery only; I/O → adapter
├── config/models.rs           ← domain types (Config, App, Preferences)
├── config/resolver.rs         ← public API + interactive ambiguity prompt
├── config/dependencies.rs     ← facade + runtime "is running?" checks
└── commands/config/validate.rs ← CLI output around DevCliConfigValidator
```

---

## Phases

| Phase | Scope | Status |
|-------|-------|--------|
| 1 | Path dep + adapter + `load_config`/`save_config` | **Done** |
| 2 | `ProjectAppResolver` + fuzzy match | **Done** |
| 3 | `DependencyGraph` (cycles error) | **Done** |
| 4 | `DevCliConfigValidator` for `config validate` | **Done** |
| 5 | Preferences via `JsonLoader`; trim loader to path discovery | **Done** |

---

## Adapter API

| Function / type | Use |
|-----------------|-----|
| `DevCliConfigManager` | config.json load/save |
| `load_preferences` / `save_preferences` | preferences.json via `JsonLoader` |
| `ProjectAppResolver` | name / `project/app` / alt names |
| `AppDependencyProvider` + `resolve_dependency_chain` | transitive deps |
| `DevCliConfigValidator` | product validation rules |

**Still product-owned:** path priority (`devcli_CONFIG_DIR` → cwd → home), interactive `Select` on ambiguous apps, `check_dependencies_running`.

**Path utils:** `expand_*` / `contract_tilde` live in `config_manager::utils`; core `utils/path` is a thin re-export.

---

## Behavior notes

- Circular dependencies error (config-manager DFS), not silent BFS truncation.
- Domain types stay in `config/models.rs`.
- Load/save round-trip preserves `~/...` shorthand: `load()` expands tilde paths
  in memory, `save()` contracts paths under `$HOME` back to `~/...` before
  writing (see `contract_app_paths` in `config_manager_support.rs`).

---

## Tests

```bash
cargo test -p config-manager
cargo test -p devcli-core config_manager_support
cargo test -p devcli-core --lib config::
cargo test -p devcli-core --lib commands::config::validate
```
