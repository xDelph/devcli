# devcli and git worktrees

Status: **implemented**. This document records the design and why it is
shaped this way; user-facing documentation lives in the README, AGENTS.md
and `docs/`.

## The problem

Git worktrees are how agent harnesses isolate sub-agents: each sub-agent
gets its own checkout (`.claude/worktrees/<name>`, `.worktrees/`, a sibling
directory, a harness directory under `~/.codex` or `~/.cursor`, ...),
created on the fly and deleted when the work is done. An app is declared
once in `~/.devcli/config.json`, with the path of one checkout. Without
support, running it from N worktrees meant N duplicated config entries,
processes colliding on ports, and processes left behind when a worktree was
deleted.

## Principles

1. **Derive, never write the config.** The set of worktrees is what git
   says (`git worktree list --porcelain`), asked on every command.
   `config.json` stays the user's file: nothing is added when a worktree
   appears, nothing is removed when it goes.
2. **The current directory decides.** A sub-agent runs plain commands from
   its worktree (`devcli start api`) and gets its worktree's copy. Nobody
   has to know the worktree's name.
3. **An instance is a project.** A worktree instance is the configured app
   with its path moved into the worktree, under the project name
   `<project>@<slug>`. Process ids (`project.app.env`), state files and log
   files all derive from the project name, so instances never collide and
   every existing code path (status, logs, restart, the daemon) works on
   them unchanged.
4. **Never cross the boundary by accident.** Nothing a sub-agent runs from
   its worktree reaches the main checkout's processes, or another
   worktree's, unless it names them.

## Facts git gives (verified)

| Question | Answer |
|---|---|
| Listing | `git worktree list --porcelain`: blocks of `worktree <path>`, `HEAD`, `branch refs/heads/<b>` or `detached`, `bare`, `locked`, `prunable` |
| Main worktree | Always listed first; absent from the list's checkouts in a bare repository |
| Unique id of a worktree | Its admin directory `<common-dir>/worktrees/<name>`. Git disambiguates the name (`feat`, `feat1`, `feat2` for three directories called `feat`) |
| Reuse | An admin name is reused once its worktree is removed |
| Same branch in two worktrees | Possible with `--force`: the branch is not a key |
| Which checkout a path is in | `git rev-parse --path-format=absolute --show-toplevel --git-common-dir` |
| Deleted with `rm -rf` | Still listed, flagged `prunable`, until `git worktree prune` |

## Slugs and slots

The slug is git's admin directory name, sanitized for process ids and log
file names (`[A-Za-z0-9_-]`). Ports need more than a name: numbering
worktrees by their position in the list would move the ports of running
apps whenever a worktree is created or removed. So each worktree gets a
**slot** the first time devcli sees it, recorded in
`<devcli dir>/worktrees.json` (runtime state, next to the process state):

- slug: the admin name, suffixed `-2`, `-3` on collision
- index: the smallest free number (the configured checkout is 0)

A slot is released once its worktree directory is gone, never merely
because a caller did not list it: another devcli may have listed the
worktrees just before a sub-agent created a new one. Updates are serialized
with a file lock, since sub-agents start apps in parallel.

## Selection

| Name | Instance |
|---|---|
| `api` | the worktree the command runs in, else the configured checkout |
| `api@feat` | the worktree whose slug is `feat`, from anywhere |
| `api@` | the configured checkout, from anywhere |
| `api@*` | every checkout that has the app (`start`, `stop`, `status`) |

The suffix works on the app or on `--project`; a name that really contains
`@` in the config is taken as is. devcli's own follow-up commands
(dependencies, `restart`, the TUI) name instances exactly (`proj@` for the
configured checkout), so the current directory never redirects them.

`stop` and `status` select processes by instance, not by app name: `stop
--project shop` run inside one of that project's worktrees selects that
worktree's processes only (`shop@*` selects all).

## What an instance gets

- Its path in the worktree (the app's path relative to its checkout,
  joined to the worktree root). An app that does not exist on the
  worktree's branch is reported (`api@feat`) or falls back to the
  configured checkout (plain `api` from inside the worktree).
- Variables: `DEVCLI_WORKTREE`, `DEVCLI_WORKTREE_INDEX`,
  `DEVCLI_PORT_OFFSET` (index × 100), `DEVCLI_WORKTREE_PATH`,
  `DEVCLI_WORKTREE_BRANCH`. The configured checkout gets the first three
  (empty, 0, 0).
- Ports: with `port` declared on the app, `PORT` and `DEVCLI_PORT` =
  port + offset. Without it, a numeric `PORT` from the env files is shifted
  in worktrees.
- Health checks on the shifted port (the configured port is replaced;
  `${DEVCLI_PORT}`-style variables are substituted).
- In Docker environments, `COMPOSE_PROJECT_NAME=<project>-<app>-<slug>`
  unless set: Compose names its project after the compose file's
  directory, the same in every worktree.
- The configured checkout's env files when the worktree has none (they are
  usually gitignored). The worktree's own env files win.
- Dependencies in the same worktree when they live in the same repository;
  shared dependencies (other repositories, databases) stay the configured
  instance.

## Lifecycle

- pm-daemon stops processes whose worktree directory no longer exists
  (`worktree_root` in the process metadata), like those of an attached
  `devcli start` that died.
- `devcli worktrees` lists repositories, worktrees, slots, branches, the
  current one and what runs where. Stale (prunable) worktrees are shown
  with a hint; devcli never runs `git worktree prune` itself.
- `auto-add` run inside a linked worktree registers the main checkout's
  copy of the directory (warning when the app only exists on the
  worktree's branch).
- The TUI lists each tracked instance as a project of its own after its
  configured project.

## Not covered

- Kubernetes: a worktree instance applies the same manifests to the same
  namespace.
- `docker run --name ...` with a fixed container name collides across
  worktrees; use the variables (`--name api-${DEVCLI_WORKTREE}`).
- Submodules: an app inside a submodule belongs to the submodule's
  repository, whose worktrees are separate.
- Installing dependencies (`node_modules`, virtualenvs) in a fresh
  worktree is the app's business (a `run` command such as `install`).
