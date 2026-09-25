# Known Issues & Possible Bugs

Working list gathered after the config-manager migration review (see
`docs/config-manager-integration.md`). Order roughly by importance. Not a
backlog — these are open items to investigate. Status fields: `[ ]` = open,
`[x]` = done.

## 1. Tilde path round-trip on save — FIXED, with one behavior change to be aware of

### The original problem

`load()` converts every `~/...` app path to a full absolute path in memory
(`~/code/app` → `/Users/thomas/code/app`). `save()` used to write the struct
*as-is*, so any load → modify → save flow would rewrite `~/...` back to
`/Users/...` in the file. That made configs non-portable (they pointed at one
machine's home dir) and mixed formats with the `~/...` paths that `auto-add` /
`detection/nx` / `app_detector_support` write.

### The fix

`DevCliConfigManager::save()` now writes a **copy** of the config through
`contract_app_paths`, which applies `contract_tilde` to every `app.path` whose
value lies **under `$HOME`**. Round-trip is now symmetric:

```
file:  ~/code/app  --load-->  memory: /Users/thomas/code/app  --save-->  file: ~/code/app
```

The caller's in-memory config is untouched (still absolute), so all code that
reads `app.path` behaves as before.

### The side effect (what changes in the file on write)

`contract_tilde` normalizes **any** path inside `$HOME` to `~/...` — including
paths the user typed as absolute. Concretely:

| What's on disk before saving | What the file contains after `save()` |
|---|---|
| `~/code/app` | `~/code/app` (unchanged, was already portable) |
| `/Users/thomas/code/app` (typed absolute by the user) | `~/code/app` (normalized) |
| `/opt/project` (outside home) | `/opt/project` (untouched) |
| `./relative` (relative path) | `./relative` (untouched) |

So the *only* observable change is: an absolute path under home is persisted as
`~/...` the first time a save happens after this fix. Purely cosmetic, no
resolution behavior changes (the value is expanded again on the next load), but
a reviewer should know the on-disk text of such paths can differ from what the
user typed. Test: `save_contracts_tilde_paths_on_roundtrip`.

## 2. The index can't "see" the callers of the new config API

### The change that triggered this

After the migration, `load_config`, `save_config`, `load_preferences`,
`save_preferences` and `resolve_dependency_chain` are thin functions in
`devcli-core` that forward to the `config-manager` crate. Their call graph
changed, and the static-analysis tool (pixel) reports it cannot track them.

### What the tool actually says

Running `pixel impact save_config` on the real function reports:

```
summary: "save_config: 0 at depth 1, 0 at depth 2, 0 at depth 3 ... risk UNKNOWN
          (lower bound: 20 unresolved same-name call sites)"
notes:    "no caller was found: the symbol is either unused or reached only
           through a reference class the index does not record ..."
epistemics.lower_bound = true
```

### Why it says that (two independent reasons)

1. **A name collision.** There are two different `save_config` functions in the
   repo: the real one in `devcli-core/src/config/loader.rs`, and an unrelated
   test helper `save_config` in `config-manager/tests/integration/async_workflows.rs`.
   The index matches call sites by symbol name, so these 20 candidate call sites
   can't be attributed to either function — hence "same-name unresolved".
2. **The index's known blind spots** (callbacks passed as arguments, dynamic
   dispatch / `obj[method]()`, macro-generated calls). Any caller that reaches
   the function through one of those channels is invisible to it.

### What it does NOT mean

- It does **not** mean the code is broken. `cargo build --workspace`,
  `cargo test --workspace --all-features` (400+ tests, 23 suites) and
  `cargo clippy -- -D warnings` all pass — if a caller were truly missing,
  compilation would fail.
- It does **not** mean the function is dead code (it has 27 real call sites
  across 8 files, listed below).

### Why it matters anyway (the real risk)

This is a *maintenance* risk, not a runtime bug: because the tool cannot see the
callers, if you later change the signature/behavior of `save_config` (or one of
the other forwarded functions), **the tool will not warn you about the callers
that need updating** — you could break `devcli config edit`, the TUI config
editor or `auto-add` silently, and only a test or a manual run would catch it.

### Known call sites (manually confirmed, 27 for `save_config`)

- `save_config` / `load_config`: `commands/config/edit.rs` (4), `commands/env.rs` (3),
  `commands/auto_add/single_app.rs` (2), `commands/auto_add/nx_monorepo.rs` (1),
  `tui/views/main_view/config_editor.rs` (9) + TUI tests (7), `config/loader.rs`.
- `load_preferences` / `save_preferences`: TUI preferences editor, `commands/env.rs`.
- `resolve_dependency_chain`: `commands/start/executor.rs` (pre-start dep auto-start).

### Recommended actions

1. **Manual smoke (highest value, lowest cost):** `devcli config edit` → edit an
   app path + a dependency → save → `devcli config validate` → `devcli start`.
   Confirms read/write and dep-resolution end-to-end through the new path.
   *Recommended before merge.*
2. **Reduce future blindness:** after merging, re-run `pixel build-index` and
   re-check `pixel impact save_config` — if the count is still `lower_bound`,
   keep the manual caller list above updated (e.g. in this file) whenever these
   functions change.
3. **Regression guard:** the round-trip test added for issue #1 already goes
   through `save_config`; a UI-level save in `config_editor.rs` is also covered
   by existing tests.
4. **Status:** `[x]` — the name collision was removed by renaming the test
   helpers in `config-manager/tests/integration/async_workflows.rs`
   (`save_config` → `save_config_async`, `load_config` → `load_config_async`, all
   call sites updated). `pixel impact save_config` should now attribute call
   sites to the real `devcli-core` function instead of reporting 20 unresolved
   same-name candidates. Re-index (`pixel build-index`) and re-check before merge.

## 3. `devcli_CONFIG_DIR` values are used literally — `~` and `$VAR` are NOT expanded

### The problem

`devcli_CONFIG_DIR` is an env var that overrides where the config file lives
(see `get_config_path` / `get_preferences_path` in `config/loader.rs`). The
value is used **verbatim** — no tilde or environment-variable expansion.

Concrete example:

```sh
export devcli_CONFIG_DIR="~/.devcli"
devcli ...        # devcli looks for a directory literally named "~"
                  # (relative to the current directory), NOT your home directory.
                  # The config silently goes to ./~/.devcli/config.json
```

It would even **create** that stray directory automatically (`fs::create_dir_all`
on the parent). Same for `$VAR`/`${VAR}`. The adapter also calls
`with_path_expansion(false)`, which is deliberate — it keeps the adapter from
re-expanding paths the resolver already handles, and is unrelated to this issue.

### Why it's currently not a big deal

This is **legacy behavior** (identical before the migration) and nothing
internal sets `devcli_CONFIG_DIR` with a shorthand. Only a user/script setting
`~` or `$VAR` in it hits this.

### Status & what was done

- **Status:** `[x]` — `~` in `devcli_CONFIG_DIR` now expands to `$HOME`.
- `get_config_path` / `get_preferences_path` read the override through a new
  `env_config_dir()` helper that applies `expand_tilde` (`config_manager::utils`)
  before use, so `devcli_CONFIG_DIR=~/.devcli` resolves under the real home.
- `$VAR` / `${VAR}` are intentionally NOT expanded (out of scope of this fix);
  only tilde shorthand is handled.
- Tests: `env_config_dir_expands_tilde_to_home`, `..._keeps_plain_absolute_path`,
  `..._none_when_unset` in `config/loader.rs`.

## 4. The ambiguous-app prompt resolves the app twice

### The problem

When an app name exists in several projects, `devcli start` (and friends) must
ask you which one you mean. It currently does the lookup **twice**:

1. First resolution pass — no project filter:
   `ProjectAppResolver::resolve()` returns `Ambiguous { candidates }` with the
   list of matching (project, app) pairs.
2. You pick one entry in the interactive `Select` prompt.
3. Second resolution pass — `resolve_in_project()` re-resolves with a
   `--project` filter to turn your choice into the final `ResolvedApp`.

Both passes search the same in-memory config. Pass 2 could simply *pick your
choice from the already-computed `candidates` list*; instead it re-runs the
matching logic (name / alternative_name / project filter) a second time. If
matching rules ever diverge between the two paths, what you selected and what
gets resolved could disagree.

Concrete example:

```sh
# two projects both contain an app named "api":
devcli start api
# → resolve() finds both → "Ambiguous"
# → you select "web/api"
# → resolve_in_project(config, "web", "api") runs again to compute the result
#    (could have just used the entry you picked)
```

This is **pre-existing** (same in the old pre-migration code), purely a
redundant computation — no user-visible bug today.

### Missing coverage

No test exercises the `alternative_name`-within-project branch (resolving by an
alias while a project filter is active).

### Status & what was done

- **Status:** `[x]` — the ambiguous-app prompt no longer re-resolves.
- `resolve_app`'s interactive branch now calls `pick_app_in_project`, which
  builds the `ResolvedApp` **directly from the config** (exact key first, then
  `alternative_name`) using the project the user just picked — the second
  `resolve_with_filter` pass is gone.
- `pick_app_in_project` mirrors `ProjectAppResolver::resolve_in_project`;
  failures are defensive only (the app came from the candidate list).
- Tests: `test_pick_app_in_project_exact_name`,
  `test_pick_app_in_project_by_alternative_name` (covers the previously missing
  `alternative_name`-within-project branch, returns the actual key, not the
  alias), `test_pick_app_in_project_missing`.
- Manual check recommended: `devcli start` on a name present in two projects,
  pick one, confirm the resolved app is the picked one.

## Housekeeping

- `tui-debug.log` (449 KB) — should stay in `.gitignore` (it is).
- `.pixel/` added to `.gitignore` (fine).
- **fmt:** repo-wide rustfmt 1.8 drift fixed in a dedicated commit
  (`style: apply rustfmt 1.8 across the whole workspace`); `cargo fmt --check`
  now passes globally.