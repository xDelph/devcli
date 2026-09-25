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
- **Status:** `[x]` — fixed on `use-config-manager` (commit after rebase)
- **What was done:** `DevCliConfigManager::save` now writes a clone of the
  config through `contract_app_paths`, which applies `contract_tilde` to every
  `app.path` **under `$HOME`** before persisting. Round-trip is now symmetric:
  file `~/x` → in-memory `$HOME/x` → file `~/x`. The caller's in-memory config
  stays expanded (consumers keep working with absolute paths); only the
  persisted copy is contracted. Paths outside `$HOME` and relative paths are
  untouched by `contract_tilde`.
- **Side effects to re-check during review:**
  - The **only** behavioral change is on-write: an `app.path` that is absolute
    **under `$HOME`** and was written that way by the user is now persisted as
    `~/...`. Intentional paths outside home and relative paths are unaffected.
  - Not applied to `preferences.json` (no paths in the model).
- **Test:** `save_contracts_tilde_paths_on_roundtrip` (load `~/my-app`, save,
  assert `~/my-app` on disk and absolute in memory).
- **Manual check (recommended):** edit an app path to `~/...` via
  `devcli config edit`, save, re-open — the file must still say `~/...`.

## 2. Unresolved same-name call sites for config API

- **Files:** `load_config`, `save_config`, `load_preferences`,
  `save_preferences`, `resolve_dependency_chain`
- **Symptom:** static analysis (pixel) reports it cannot resolve all callers of
  these symbols (lower-bound, risk flagged CRITICAL). This is a **static-analysis
  limitation, not evidence of breakage**: the whole workspace builds and
  `cargo test --workspace --all-features` passes (400+ tests across 23 suites).
  What it means concretely: callers passed as function values, behind trait
  objects, or through generated code are not resolved by the index, so a
  signature/behavior change to these functions can ship unnoticed by the graph.
- **Known call sites (manually confirmed):**
  - `load_config`/`save_config` → `config/loader.rs`, then
    `commands/config/edit.rs`, `commands/env.rs`,
    `tui/views/main_view/config_editor.rs` (4 call sites), `commands/auto_add/nx_monorepo.rs`.
  - `load_preferences`/`save_preferences` → TUI prefs editor, `commands/env.rs`.
  - `resolve_dependency_chain` → `commands/start/executor.rs` (and `pre`-start dep auto-start).
- **Status:** `[ ]`
- **Recommended actions (in order of value):**
  1. **Manual smoke (low effort, high value):** `devcli config edit` → change an
     app path + a dependency → save → `devcli config validate` → `devcli start` —
     confirms read/write formatting and dep-chain resolution end-to-end on the
     new config-manager path.
  2. **Green the resolver:** re-run `pixel build-index` after finalizing the
     branch and re-check `pixel impact "save_config"` — the unresolved-caller
     count should drop; if `lower_bound` is still true, add explicit integration
     coverage for the flows above rather than trusting the graph.
  3. **Regression guard:** the round-trip test added for issue #1 now exercises
     `save_config` through the adapter; consider also a test driving
     `config_editor.rs`'s save path if it isn't already covered by a UI test.

## 3. `devcli_CONFIG_DIR` env-var paths are used raw (no `~`/`$VAR` expansion)

- **File:** `devcli-core/src/config/loader.rs` (`get_config_path` /
  `get_preferences_path`) + adapter (`with_path_expansion(false)`)
- **Symptom:** if `devcli_CONFIG_DIR` contains `~` or `$VAR`, the value is used
  verbatim (e.g. `~/.devcli` → literal directory named `~`). Behavior is
  **identical to the legacy pre-migration loader** — the migration did not
  change this. The `with_path_expansion(false)` in the adapter is deliberate:
  it keeps the adapter from re-expanding paths the resolver already handled.
- **Who is affected:** only users/scripts that set `devcli_CONFIG_DIR` with a
  shell-style shorthand. No known internal usage does this.
- **Status:** `[ ]` (verify intent, low priority)
- **Recommended actions:**
  1. **Decide the desired contract** for `devcli_CONFIG_DIR`: either document
     "absolute paths only" (current reality) or expand `~`/`$VAR` once at the
     top of `get_config_path`/`get_preferences_path` (e.g. reuse
     `config_manager::utils::expand_path`), keeping `with_path_expansion(false)`
     in the adapter regardless.
  2. If expansion is kept disabled, add one line in `docs/config-manager-integration.md`
     stating that `devcli_CONFIG_DIR` is taken raw.
  3. Cheap regression: a unit test asserting a literal `~/.devcli` in
     `devcli_CONFIG_DIR` either expands or is taken verbatim, locking in the
     decision.

## 4. Ambiguous-app prompt does double resolution

- **File:** `devcli-core/src/config/resolver.rs` (`resolve_app`) +
  `ProjectAppResolver` (`resolve` / `resolve_with_filter`)
- **Symptom:** app resolution is attempted **twice** in the ambiguous case.
  Pass 1: `resolve()` with no project filter returns
  `Err(Error::Ambiguous { candidates })`. The interactive `Select` then runs
  pass 2: if the user's choice resolves inside a single project, it re-resolves
  with a `--project` filter (`resolve_in_project`). Two resolution passes where
  one would do; the second pass's result could be derived directly from the
  already-computed `candidates`. Behavior matches the old (pre-migration) code,
  so this is a pre-existing inefficiency, not a regression.
- **Missing coverage:** no dedicated test for the `alternative_name`-within-
  project branch (resolving by `alternative_name` while a project filter is
  active).
- **Status:** `[ ]` (cosmetic / test coverage, no user-visible bug)
- **Recommended actions:**
  1. **Cheap correctness win (do first):** add a unit test for
     `resolve_in_project` resolving by `alternative_name` and by exact name,
     plus a test asserting `resolve_with_filter` with no filter still returns
     `Ambiguous` — protects the existing behavior before any optimization.
  2. **Optional optimization:** change the prompt handler to consume the
     already-fetched `candidates` (pick the chosen `(project, app)` from the
     list) instead of re-running `resolve_in_project` with a filter — one
     unification pass, same output. Low risk, but needs a UI/functional check
     of the interactive `devcli start` prompt.
  3. Leave unresolved symbols in this path alone until #2 lands; there are no
     known users that depend on double resolution.

## Housekeeping

- `tui-debug.log` (449 KB) checked out of `.gitignore`? — should stay ignored.
- `.pixel/` added to `.gitignore` (fine).
- **fmt drift:** the repository has a large pre-existing rustfmt-1.8.0 drift
  (~90 files) inherited from an older formatter version. The config-manager
  branch files are now rustfmt-clean (validated with `skip_children=true` for
  module roots); a repo-wide `cargo fmt --all` commit is still needed before the
  CI `fmt` gate can pass globally.