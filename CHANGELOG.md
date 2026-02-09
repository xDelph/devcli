# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Structured logging with tracing crate
  - JSON logs written to `~/.devcli/logs/devcli.YYYY-MM-DD.json`
  - Daily log rotation
  - Environment-based filtering via `RUST_LOG`
  - Spans for operation tracking
- Metrics collection system
  - HTTP JSON API on `localhost:9090/metrics`
  - `devcli metrics` CLI command for formatted display
  - Tracks process, system, and performance metrics
  - In-memory aggregation with automatic pruning
- GitHub Actions workflows
  - CI workflow for tests, linting, and builds
  - Release workflow for multi-platform binary builds
- Comprehensive documentation
  - Release process guide
  - Metrics implementation summary

### Changed
- Improved logging throughout codebase (150+ locations)
- Better error messages with structured context
- Monitor daemon now starts metrics server automatically

### Deprecated
- `FileLogger` - Use tracing infrastructure instead
- `MonitorLogger` - Use tracing spans and events instead

## [0.1.0] - 2024-XX-XX

### Added
- Initial release
- Config-based process management
- Dependency resolution and auto-start
- Local and Docker environment support
- Process health checks
- Automatic restart on crash
- Process state tracking
- TUI interface for log viewing
- Monitor daemon for process supervision
- Commands: start, stop, restart, run, status, monitor, health-check
- Auto-detection for Node.js, Nx monorepos, Docker, and Kubernetes apps
- Environment file management
- User preferences system

[Unreleased]: https://github.com/YOUR_USERNAME/devcli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/YOUR_USERNAME/devcli/releases/tag/v0.1.0
