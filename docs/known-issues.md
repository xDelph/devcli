# Known Issues & Possible Bugs

Quick working list gathered after the config-manager migration review (see
`docs/config-manager-integration.md`). Order roughly by importance. Not a
backlog — these are open items to investigate. Status fields: `[ ]` = open,
`[x]` = done.

## 1. Tilde path round-trip on save (`~/...` rewritten to absolute paths)

- **File:** `devcli-core/src/config_manager_support.rs` (`DevCliConfigManager::load` / `save`)
- **Symptom:** `load()` expands `~` → absolute in memory; `save()` writes the
  struct as-is. Any load → modify → save flow (`devcli config edit`, `devcli env`,
  TUI config editor, `auto-add`) rewrites every user `~/...` path to
  `/Users/...`, destroying portability and mixing formats with the
  `contract_tilde`-written paths produced by `auto-add` / `detection/nx` /
  `app_detector_support`.
- **Status:** `[ ]`
- **Idea / lowest-risk fix:** apply `contract_tilde` to app paths in the
  adapter's `save` path (symmetric to `expand_app_paths` on load); or stop
  expanding on load and expand lazily at point of use. Pre-existing on develop
  (old loader did the same), but the migration is the right time to fix it.

## 2. Unresolved same-name call sites for config API

- **Files:** `load_config`, `save_config`, `load_preferences`,
  `save_preferences`, `resolve_dependency_chain`
- **Symptom:** static analysis (pixel) reports it cannot resolve all callers of
  these symbols (lower-bound, risk flagged CRITICAL). Full workspace build +
  400+ tests pass, so nothing known is broken — but the new save path is only
  exercised via round-trip tests in `tui/views/main_view/config_editor.rs`,
  `commands/env.rs`, `commands/config/edit.rs`.
- **Status:** `[ ]`
- **Action:** manual smoke: `devcli config edit` (change + save) then
  `devcli config validate`; confirm read/write formatting end-to-end.

## 3. `devcli_CONFIG_DIR` env-var paths are used raw (no `~`/`$VAR` expansion)

- **File:** `devcli-core/src/config/loader.rs` (+ adapter uses
  `with_path_expansion(false)`)
- **Symptom:** a `~` or `$VAR` in `devcli_CONFIG_DIR` won't be expanded.
  Consistent with legacy behavior, so likely intended — just check whether any
  users/scripts rely on shorthand in that variable.
- **Status:** `[ ]` (verify intent, low priority)

## 4. Ambiguous-app prompt does double resolution

- **File:** `devcli-core/src/config/resolver.rs` (+ `ProjectAppResolver`)
- **Symptom:** `resolve()` first returns `Ambiguous`, then the interactive
  `Select` re-resolves with a `--project` filter — two resolution passes where
  one would do. Behavior matches the old code; missing dedicated test for the
  alternative_name-within-project branch.
- **Status:** `[ ]` (cosmetic / test coverage)

## Housekeeping

- `tui-debug.log` (449 KB) checked out of `.gitignore`? — should stay ignored.
- `.pixel/` added to `.gitignore` (fine).