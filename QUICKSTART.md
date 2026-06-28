# devcli - Quick Start Guide

## Installation

### Install Rust (if not already installed)
```bash
brew install rustup-init
rustup-init
source ~/.cargo/env
```

### Build devcli
```bash
cd /Users/thomas.delalonde/Projects/perso/devcli
cargo build --release
```

The binary will be at: `./target/release/devcli`

## Basic Usage

### Start a Process (Attached Mode)
Attached mode displays logs in real-time:
```bash
./target/release/devcli start my-app --cmd "node server.js"
```

### Start a Process (Detached Mode)
Detached mode runs in background:
```bash
./target/release/devcli start my-app --cmd "node server.js" --detach
```

### With Custom Directory
```bash
./target/release/devcli start my-api \
  --cmd "python app.py" \
  --dir /path/to/project
```

### With Environment Variables
```bash
./target/release/devcli start my-service \
  --cmd "node server.js" \
  --env PORT=3000 \
  --env NODE_ENV=production \
  --detach
```

### Check Process Status
All processes:
```bash
./target/release/devcli status
```

Specific process:
```bash
./target/release/devcli status my-app
```

## File Locations

### Log Files
```
~/.devcli/logs/<app-name>_<timestamp>.log
```

Example:
```bash
tail -f ~/.devcli/logs/my-app_20251024_120000.log
```

### Process State Files
```
~/.devcli/processes/<project>.<app>.<environment>.json
~/.devcli/processes/.status_changed
```

Install `pm-daemon` alongside `devcli` for health monitoring and auto-restart.

## Common Workflows

### Development Server
```bash
# Start in attached mode to see logs
./target/release/devcli start dev \
  --cmd "npm run dev" \
  --dir ./my-project
```

### Background Service
```bash
# Start detached
./target/release/devcli start api \
  --cmd "python api.py" \
  --dir ./backend \
  --env FLASK_ENV=production \
  --detach

# Check if running
./target/release/devcli status api

# View logs
tail -f ~/.devcli/logs/api_*.log
```

### Multiple Services
```bash
# Start multiple services
./target/release/devcli start frontend --cmd "npm start" --dir ./frontend --detach
./target/release/devcli start backend --cmd "npm start" --dir ./backend --detach
./target/release/devcli start worker --cmd "npm start" --dir ./worker --detach

# Check all
./target/release/devcli status
```

## Tips

### Install Globally
```bash
cargo install --path devcli
devcli --help
```

### Alias for Convenience
Add to your `~/.zshrc`:
```bash
alias rcli="/Users/thomas.delalonde/Projects/perso/devcli/target/release/devcli"
```

Then use:
```bash
rcli start my-app --cmd "echo hello" --detach
rcli status
```

### Complex Commands
For commands with complex shell syntax, create a script:

**scripts/start-dev.sh:**
```bash
#!/bin/bash
export NODE_ENV=development
npm install
npm run dev
```

Then:
```bash
chmod +x scripts/start-dev.sh
./target/release/devcli start dev --cmd "./scripts/start-dev.sh" --detach
```

## Troubleshooting

### Process Won't Start
```bash
# Check if app name is already in use
./target/release/devcli status my-app

# Try a different name or wait for old process to finish
```

### Can't Find Logs
```bash
# List all log files
ls -lh ~/.devcli/logs/

# Find specific app logs
ls ~/.devcli/logs/my-app_*
```

### Process Shows as Dead
```bash
# Status command auto-cleans dead processes
# Just run status again, it will be removed
./target/release/devcli status
```

## Next Steps

- Read [README.md](README.md) for full documentation
- Check [DEVELOPMENT.md](DEVELOPMENT.md) for contributing
- See [PROJECT_RULES.md](PROJECT_RULES.md) for architecture

## Help

```bash
./target/release/devcli --help
./target/release/devcli start --help
./target/release/devcli status --help
```

