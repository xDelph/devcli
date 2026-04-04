# devcli-core Command Flow Diagrams (CLI + TUI)

This file is diagrams-only, to validate flow behavior for:

1. `start`
2. `run`
3. `stop`
4. `restart`

Each command has:

- one **CLI** flowchart
- one **TUI** flowchart
- explicit branches for relevant flags
- explicit branch for output streaming vs non-streaming

## 1) `start` CLI

```mermaid
flowchart TD
  A["CLI input: devcli start <apps...> [--project] [--env] [--skip-deps] [--stage]"] --> B{"app_names empty?"}
  B -- "yes" --> B1["Error: at least one app required"]
  B -- "no" --> C["Resolve apps + environment<br/>env = --env or preferences.default_env"]
  C --> D{"--skip-deps?"}
  D -- "yes" --> F["Skip dependency handling"]
  D -- "no" --> E["Handle dependencies for all target apps"]
  E --> E1{"Missing deps?"}
  E1 -- "no" --> F
  E1 -- "yes + auto_start_deps=true" --> E2["Start missing dependencies in parallel"]
  E1 -- "yes + auto_start_deps=false" --> E3["Error: missing dependencies"]
  E2 --> F
  F --> G["Filter apps that became already-running"]
  G --> H["Start remaining apps in parallel<br/>spawn internal-spawner per app"]
  H --> I["Persist logs always<br/>~/.devcli/logs/{project}_{app}_{env}_{date}.log"]
  H --> J{"preferences.detached_mode?"}
  J -- "true" --> J1["No live app log stream in terminal<br/>start exits quickly"]
  J -- "false" --> J2["Live app output streamed to terminal<br/>until Ctrl+C or all started apps exit"]
  J1 --> K["Ensure monitor daemon running"]
  J2 --> K
  K --> L["Done"]
```

## 2) `start` TUI

```mermaid
flowchart TD
  A["Status tab: key 's' on stopped app"] --> B["AppState.set_env_selection_requested()"]
  B --> C["PopupManager opens Start popup with env selector"]
  C --> D["User selects env with ←/→ and presses Enter"]
  D --> E["CommandExecutor runs:<br/>devcli start <app> --project <project> --env <selected_env>"]
  E --> F["Popup streams CLI stdout/stderr as LogLine events"]
  E --> G["Same core start flow as CLI path"]
  G --> H["Logs persisted always in ~/.devcli/logs/..."]
  G --> I{"preferences.detached_mode?"}
  I -- "true" --> I1["Popup receives short startup output only<br/>no continuous app stream"]
  I -- "false" --> I2["Popup receives continuous streamed output<br/>while start command remains attached"]
  I1 --> J["User closes popup (Enter/Esc)"]
  I2 --> J
  N["TUI-exposed flags for start:<br/>--env (via popup selector)<br/>Not exposed: --skip-deps, --stage"] -.-> E
```

## 3) `run` CLI

```mermaid
flowchart TD
  A["CLI input: devcli run <app> <variant> [--project] [--env] [--skip-deps]"] --> B["Resolve app + environment + command variant"]
  B --> C{"Variant exists in env?"}
  C -- "no" --> C1["Error: command variant not found"]
  C -- "yes" --> D{"--skip-deps?"}
  D -- "yes" --> F["Skip dependency handling"]
  D -- "no" --> E["Handle dependencies (same helper as start)"]
  E --> F
  F --> G["Prepare command + env vars<br/>(env file, docker/orbstack transforms, stage defaults)"]
  G --> H["spawn_process(detached=true, show_output=!detached_mode)"]
  H --> I["Persist logs always<br/>~/.devcli/logs/{project}_{app}_{env}_{date}.log"]
  H --> J["Sleep 2s and check process still running"]
  J --> K{"Still running?"}
  K -- "no" --> K1["Treat as completed quickly<br/>no long-lived tracker registration"]
  K -- "yes" --> K2["Register process metadata + ensure monitor running"]
  K1 --> L{"preferences.detached_mode?"}
  K2 --> L
  L -- "true" --> L1["No continuous terminal stream"]
  L -- "false" --> L2["Stream output in terminal<br/>until Ctrl+C or process exits"]
  L1 --> M["Done"]
  L2 --> M
```

## 4) `run` TUI

```mermaid
flowchart TD
  A["Config tab: select command and press Enter"] --> B["AppState.set_command_execution_requested(command_idx)"]
  B --> C["PopupManager opens Run confirmation popup"]
  C --> D["User confirms with Enter"]
  D --> E["CommandExecutor runs:<br/>devcli run <app> <command> --project <project> --env <env_from_command>"]
  E --> F["Popup streams CLI stdout/stderr as LogLine events"]
  E --> G["Same core run flow as CLI path"]
  G --> H["Logs persisted always in ~/.devcli/logs/..."]
  G --> I{"preferences.detached_mode?"}
  I -- "true" --> I1["Popup gets short output only"]
  I -- "false" --> I2["Popup gets continuous output while run stays attached"]
  I1 --> J["User closes popup"]
  I2 --> J
  N["TUI-exposed flags for run:<br/>no extra flag controls<br/>Not exposed: --skip-deps"] -.-> E
```

## 5) `stop` CLI

```mermaid
flowchart TD
  A["CLI input: devcli stop [app_name] [--project] [--all] [--force]"] --> B["Cleanup dead entries + list tracked processes"]
  B --> C{"Selection mode"}
  C -- "--all" --> C1["Select all running tracked processes"]
  C -- "app_name (+ optional --project)" --> C2["Select matching app process"]
  C -- "--project only" --> C3["Select all app processes in project"]
  C -- "none matched" --> C4["No-op message and return"]
  C1 --> D
  C2 --> D
  C3 --> D
  D{"--force?"}
  D -- "no" --> E["Send SIGTERM to parent + recursive children"]
  E --> E1{"Exited within ~5s?"}
  E1 -- "yes" --> G["Remove PID metadata"]
  E1 -- "no" --> F["Fallback SIGKILL parent + children"]
  D -- "yes" --> F
  F --> G["Remove PID metadata"]
  G --> H["Summary output"]
  H --> I["Done"]
  Z["Streaming behavior:<br/>no app log streaming; command prints stop progress lines only"] -.-> H
```

## 6) `stop` TUI

```mermaid
flowchart TD
  A["Status tab: key 's' on running app"] --> B["AppState.set_stop_requested()"]
  B --> C["PopupManager opens Stop popup"]
  C --> D["Popup auto-executes immediately (no confirm step)"]
  D --> E["CommandExecutor runs:<br/>devcli stop <app> --project <project>"]
  E --> F["Popup streams CLI stdout/stderr until command exits"]
  E --> G["Same core stop flow as CLI path"]
  G --> H["Process terminated + PID metadata removed"]
  H --> I["No further app output stream after process stop"]
  I --> J["User closes popup"]
  N["TUI-exposed flags for stop:<br/>none<br/>Not exposed: --all, --force"] -.-> E
```

## 7) `restart` CLI

```mermaid
flowchart TD
  A["CLI input: devcli restart <app> [--project] [--env] [--skip-deps]"] --> B["Resolve app + locate running tracked process"]
  B --> C{"Process currently running?"}
  C -- "no" --> C1["Error: process not running; use start"]
  C -- "yes" --> D["Resolve restart env:<br/>--env override else running env else default_env"]
  D --> E["Stop existing process via stop_command"]
  E --> F{"Previous command_variant != default variant for env?"}
  F -- "yes" --> G["Restart path = run_command<br/>passes --skip-deps semantics"]
  F -- "no" --> H["Restart path = start_command<br/>reuses previous stage metadata"]
  G --> I["Logs persisted always in ~/.devcli/logs/..."]
  H --> I
  I --> J{"preferences.detached_mode?"}
  J -- "true" --> J1["Short non-streaming completion output"]
  J -- "false" --> J2["Live streamed output from underlying run/start path"]
  J1 --> K["Done"]
  J2 --> K
```

## 8) `restart` TUI

```mermaid
flowchart TD
  A["Status tab: key 'r' on running app"] --> B["AppState.set_restart_requested()"]
  B --> C["PopupManager opens Restart popup"]
  C --> D["Popup auto-executes immediately (no confirm step)"]
  D --> E["Env passed from tracked process env<br/>(fallback: local)"]
  E --> F["CommandExecutor runs:<br/>devcli restart <app> --project <project> --env <env>"]
  F --> G["Popup streams CLI stdout/stderr as LogLine events"]
  F --> H["Same core restart flow as CLI path"]
  H --> I{"Restart internal path"}
  I -- "run path" --> I1["Uses run_command internals"]
  I -- "start path" --> I2["Uses start_command internals"]
  I1 --> J["Logs persisted always in ~/.devcli/logs/..."]
  I2 --> J
  J --> K{"preferences.detached_mode?"}
  K -- "true" --> K1["Short popup output only"]
  K -- "false" --> K2["Continuous popup stream while command remains attached"]
  K1 --> L["User closes popup"]
  K2 --> L
  N["TUI-exposed flags for restart:<br/>--env is implicit from running process<br/>Not exposed: --skip-deps"] -.-> F
```

