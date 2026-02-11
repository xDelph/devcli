# Images Needed for DevCLI Website

## Hero Section

### 1. Terminal Demo Animation (CRITICAL - Hero)
**File**: `hero-terminal-demo.gif` or `hero-terminal-demo.mp4`
**Dimensions**: 1920x1080 (16:9)
**Format**: Animated GIF or MP4
**Content**: Show a complete flow:
```bash
# Terminal starts
$ devcli start api frontend database
✓ Starting api...
✓ Starting frontend...
✓ Starting database...
✓ Successfully started 3 app(s)

$ devcli status
# Show colorful table with running processes

$ devcli ui
# Briefly show TUI opening
```
**Notes**:
- Use dark terminal theme (match the site design)
- Show actual colored output
- Keep it under 5 seconds, looping
- Use tools like [asciinema](https://asciinema.org/) or [terminalizer](https://terminalizer.com/)

---

## TUI Screenshots

### 2. TUI Main Dashboard (HIGH PRIORITY)
**File**: `tui-dashboard.png`
**Dimensions**: 1920x1080 minimum
**Content**:
- Main process list view
- Show 3-5 running apps
- Status indicators (●green, ○gray)
- Uptime information
- PID numbers
- Navigation hints at bottom
- Keyboard shortcuts visible

**Example layout**:
```
┌─ DevCLI Monitor ─────────────────────────────┐
│ Status │ Config │ Logs                        │
├─────────────────────────────────────────────┤
│ ● api-backend          PID: 12345  Up: 2h 3m │
│ ● frontend             PID: 12346  Up: 2h 3m │
│ ● database             PID: 12347  Up: 2h 5m │
│ ○ redis                Stopped              │
├─────────────────────────────────────────────┤
│ ↑/↓: Navigate  Enter: Details  q: Quit      │
└─────────────────────────────────────────────┘
```

### 3. TUI Log Viewer (HIGH PRIORITY)
**File**: `tui-logs.png`
**Dimensions**: 1920x1080 minimum
**Content**:
- Multi-app log streaming
- ANSI colors visible
- Scrollable content
- App name tags for each log line
- Different colored timestamps
- Show real application logs (not fake/generic text)

**Example**:
```
[api][10:23:45] Starting server on port 3000
[frontend][10:23:46] Compiled successfully
[database][10:23:47] Database connection established
[api][10:23:48] GET /health 200
```

### 4. TUI Config View
**File**: `tui-config.png`
**Dimensions**: 1920x1080 minimum
**Content**:
- YAML configuration displayed
- Syntax highlighting
- Show a complete app configuration
- Scrollable view indicator

---

## Terminal Command Screenshots

### 5. `devcli status` Output (MEDIUM PRIORITY)
**File**: `terminal-status.png`
**Dimensions**: 1200x800 minimum
**Content**:
- Colorful table output
- Multiple running processes
- Health check status (✓✗indicators)
- Uptime information
- PID and project info
- Use actual terminal with real output

### 6. `devcli start` Multiple Apps (MEDIUM PRIORITY)
**File**: `terminal-start.png`
**Dimensions**: 1200x800 minimum
**Content**:
```bash
$ devcli start api frontend database
→ Starting 'api' in /workspace/backend (environment: local)
  Command: npm run dev
  Log file: /Users/you/.devcli/logs/project_api_local_20240210.log
→ Starting 'frontend' in /workspace/frontend (environment: local)
  Command: npm run dev
  Log file: /Users/you/.devcli/logs/project_frontend_local_20240210.log
→ Starting 'database' in /workspace/db (environment: docker)
  Command: docker compose up database
  Log file: /Users/you/.devcli/logs/project_database_docker_20240210.log

✓ Successfully started 3 app(s): api, frontend, database
```

### 7. `devcli metrics` Output
**File**: `terminal-metrics.png`
**Dimensions**: 1200x800 minimum
**Content**:
```
=== Process Metrics ===
Total Processes:       5
  Running:             4 ●
  Stopped:             1 ○

Total Restarts:        2
Restarts (last hour):  1
Health Check Success:  98.5%

=== System Metrics ===
Monitor Uptime:        1234 seconds (20 minutes)
Loop Iterations:       412
DevCLI Version:        0.1.0

=== Performance Metrics ===
Avg Startup Time:      142.50 ms
Avg Health Check:      23.45 ms
```

---

## Configuration Examples

### 8. YAML Configuration Example (HIGH PRIORITY)
**File**: `config-example.png`
**Dimensions**: 1200x900 minimum
**Content**:
- Well-formatted YAML
- Syntax highlighting
- Show complete example with:
  - Multiple commands
  - Health checks
  - Restart policy
  - Dependencies
  - Comments explaining features
- Use a code editor screenshot (VS Code with nice theme)

**Example content**:
```yaml
projects:
  my-project:
    apps:
      api:
        path: ./backend
        commands:
          local:
            start: npm run dev
            build: npm run build

        # HTTP health check
        health_check:
          type: http
          url: http://localhost:3000/health
          interval_secs: 10

        # Auto-restart on crash
        restart_policy:
          max_restarts: 5
          exponential_backoff: true

        # Dependencies
        dependencies:
          - database
          - redis
```

---

## Feature Illustrations (OPTIONAL BUT NICE)

### 9. Auto-add Interactive Session
**File**: `auto-add-interactive.png`
**Dimensions**: 1200x800
**Content**:
```bash
$ devcli auto-add

Scanning workspace for apps...
Found 5 apps:

✓ api (Node.js app detected)
✓ frontend (React app detected)
✓ database (Docker Compose detected)
✓ redis (Docker Compose detected)
✓ worker (Python app detected)

? Select apps to add: (Press <space> to select)
❯ ◉ api
  ◉ frontend
  ◉ database
  ◯ redis
  ◉ worker
```

### 10. Health Check Monitoring
**File**: `health-check-monitoring.png`
**Dimensions**: 1200x800
**Content**:
- Show logs or TUI with health check activity
- Health check failures
- Automatic restart trigger
- Recovery messages

### 11. Dependency Resolution
**File**: `dependency-resolution.png`
**Dimensions**: 1200x800
**Content**:
```bash
$ devcli start frontend

Resolving dependencies...
  frontend depends on:
    → api
    → api depends on:
        → database
        → redis

Starting in order:
  1. ✓ database
  2. ✓ redis
  3. ✓ api
  4. ✓ frontend

All apps started successfully!
```

---

## Social Media / Open Graph (OPTIONAL)

### 12. OG Image for Social Sharing
**File**: `og-image.png`
**Dimensions**: 1200x630 (Open Graph standard)
**Content**:
- DevCLI logo/name
- Tagline: "Powerful Process Management CLI"
- Clean, professional design
- Use brand colors (blue/cyan gradient)

---

## Architecture Diagram (OPTIONAL)

### 13. How DevCLI Works
**File**: `architecture-diagram.png`
**Dimensions**: 1600x1200
**Content**:
- Visual flowchart showing:
  - DevCLI CLI
  - Background Monitor Daemon
  - Managed Processes
  - Health Check Engine
  - Metrics Collector
- Clean, modern design
- Use icons and arrows
- Match site color scheme

---

## Priority Guide

### MUST HAVE (Launch Blockers):
1. ✅ **Hero terminal demo** - First impression
2. ✅ **TUI dashboard** - Main feature showcase
3. ✅ **TUI logs** - Key feature
4. ✅ **Config example** - Show simplicity

### SHOULD HAVE (Launch with these if possible):
5. ✅ **Terminal status output**
6. ✅ **Terminal start output**
7. ✅ **Terminal metrics output**

### NICE TO HAVE (Can add later):
8. Auto-add interactive
9. Health check monitoring
10. Dependency resolution
11. Architecture diagram
12. OG image

---

## Tools for Creating Screenshots

### Terminal Recording:
- **asciinema** - Record terminal sessions, export to GIF
- **terminalizer** - Create animated terminal GIFs
- **ttygif** - Convert terminal recordings to GIFs
- **VHS** (by Charm) - Programmatic terminal recordings

### Screenshot Tools:
- **Shottr** (macOS) - Quick screenshots with annotations
- **Flameshot** (Linux) - Feature-rich screenshot tool
- **Carbon** (web) - Beautiful code screenshots
- **Ray.so** (web) - Another code screenshot tool

### Terminal Themes:
- Use a dark theme (e.g., Dracula, Nord, Tokyo Night)
- Match the website's color scheme (blues/cyans)
- Ensure good contrast for readability

---

## File Naming Convention

Save all files in `website/public/` with these names:
- `hero-terminal-demo.gif`
- `tui-dashboard.png`
- `tui-logs.png`
- `tui-config.png`
- `terminal-status.png`
- `terminal-start.png`
- `terminal-metrics.png`
- `config-example.png`
- `auto-add-interactive.png` (optional)
- `health-check-monitoring.png` (optional)
- `dependency-resolution.png` (optional)
- `architecture-diagram.png` (optional)
- `og-image.png` (optional)

---

## After Adding Images

Replace the placeholder divs in the components with actual images:

```astro
<!-- Before -->
<div class="aspect-video bg-gradient-to-br from-slate-800 to-slate-900 rounded-lg flex items-center justify-center">
  <p>Image placeholder</p>
</div>

<!-- After -->
<img
  src="/hero-terminal-demo.gif"
  alt="DevCLI terminal demonstration"
  class="w-full h-full object-cover rounded-xl shadow-2xl"
/>
```
