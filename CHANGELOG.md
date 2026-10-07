# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0](https://github.com/xDelph/devcli/compare/v0.1.0...v0.2.0) (2026-10-07)


### Features

* add alternative_name for apps and projects ([c3e5323](https://github.com/xDelph/devcli/commit/c3e5323ed44c8d4a17ad91468ba295feea1848ef))
* add deterministic color assignment for app names ([9be6d12](https://github.com/xDelph/devcli/commit/9be6d12cfdbe12e0d0832903309e7368ba54926c))
* add LOG_LEVEL environment variable for tracing control ([c176cd4](https://github.com/xDelph/devcli/commit/c176cd4b06c308abd2af3cf1f7268df3f1aed591))
* **app-detector:** add app-detector crate ([c9b6539](https://github.com/xDelph/devcli/commit/c9b6539574c7c9d67bcaa4e75e6bc4445ddc8086))
* **app-detector:** add package manager and workspace utilities ([3897e2f](https://github.com/xDelph/devcli/commit/3897e2fb4ed25598a2ee1d0c3c87910d583aa8f0))
* **app-detector:** detect node package manager in nodejs strategy ([d818149](https://github.com/xDelph/devcli/commit/d818149aed08587faec798dbb7342b2324caeae9))
* **app-detector:** detect redis-only docker-compose projects ([9424988](https://github.com/xDelph/devcli/commit/942498883b01217e2965fa78296089646e6282bc))
* **app-detector:** detect rust and python workspaces ([d138c8e](https://github.com/xDelph/devcli/commit/d138c8ef7e0fb00f4819ceab5ff0d175adcf4802))
* **app-detector:** emit workspace and PM-aware local-env commands ([004e0d8](https://github.com/xDelph/devcli/commit/004e0d89a307c17b36b0238783bb920755b0aebe))
* **app-detector:** fix bug, add nx env and lots of fixtures with report ([105b626](https://github.com/xDelph/devcli/commit/105b626a551526770681566614c67919446ada71))
* **app-detector:** migrate env command generation to strategies ([78044bd](https://github.com/xDelph/devcli/commit/78044bd25367fd41c337a2a895bafede8e41660b))
* **cli:** agent-friendly JSON output, logs command and exit codes ([28eafd3](https://github.com/xDelph/devcli/commit/28eafd3712a9ca1d375627934a5001a14d5a48af))
* **config:** create the config-manager crate ([af3d4cb](https://github.com/xDelph/devcli/commit/af3d4cba3734f8e4c83629f63e6571809d81b6a5))
* **devcli-core:** add app-detector dependency and support adapter ([c625abb](https://github.com/xDelph/devcli/commit/c625abbbeb6787372433578d078118abd6b936ba))
* **devcli-core:** complete env-flow runtime integration ([2568055](https://github.com/xDelph/devcli/commit/256805562f860f08b0d35c49a1404d71ed0fce93))
* **devcli-core:** consolidate env runtime loaders in env-flow adapter ([6af9804](https://github.com/xDelph/devcli/commit/6af98049710a711d63533edb257d510e7da6ca58))
* **devcli-core:** improve status polling and runtime visibility ([2c20ae1](https://github.com/xDelph/devcli/commit/2c20ae19c35844f98c0d6e93272ca9bd57ff74e7))
* **devcli-core:** integrate env-flow parser via adapter ([1734487](https://github.com/xDelph/devcli/commit/173448732895a9f6240270effffcd7dae36b6683))
* **devcli-core:** route docker/orbstack env files through env-flow ([710c24b](https://github.com/xDelph/devcli/commit/710c24b9d4dd35b83b7be74ba81af078664b162e))
* **devcli-core:** use env-flow cascade for local prepare ([b35bb9f](https://github.com/xDelph/devcli/commit/b35bb9f2e2ae6d28bf7ced96cbff8617b4e17440))
* **env-flow:** new env-flow crate ([9fdf8c6](https://github.com/xDelph/devcli/commit/9fdf8c62fdca5a23c96dcda588d1231fe5aee6e8))
* **env-flow:** support reverse env filenames and cascade export ([50086c9](https://github.com/xDelph/devcli/commit/50086c98353baf951b87cf692bb492e302faf346))
* prioritize current directory .devcli over home directory ([5921f84](https://github.com/xDelph/devcli/commit/5921f8483e723279461049554b69d156b396d0f6))
* **process-manager:** create the process-manager crate ([51d8323](https://github.com/xDelph/devcli/commit/51d832300f0f06d61cc74e145530b35eebaaf424))
* **process-manager:** expand engine, StateStore and daemon discovery ([1994126](https://github.com/xDelph/devcli/commit/1994126e0cb715e9f7c0285e7165f30e4716d502))
* **process-manager:** update last things and create migration script ([67097c0](https://github.com/xDelph/devcli/commit/67097c06df7daae90b25fd287a94a639503bc930))
* refacto to use process-manager ([3859d60](https://github.com/xDelph/devcli/commit/3859d60aafd16b58f46f2eb1392c67066da7ba87))
* update monitor loop and add tests ([3f71f1b](https://github.com/xDelph/devcli/commit/3f71f1b1566ec8ece2bcd0535b57109441a5d63d))
* **website:** add the website ([d9915ba](https://github.com/xDelph/devcli/commit/d9915ba0e4a6d9f5cfbaffe1ae68078957e3eb02))
* **website:** improve mobile navigation in docs ([370ee27](https://github.com/xDelph/devcli/commit/370ee27d1f803ba842bbef6b4a7125e8bfb7a589))
* **website:** update UX ([e2dfd4f](https://github.com/xDelph/devcli/commit/e2dfd4f86b7a73852b2497eb0fc0754335d5ab68))


### Bug Fixes

* **app-detector:** detect k8s suffix manifests in kubernetes-env ([e1f13e6](https://github.com/xDelph/devcli/commit/e1f13e6664ebb157dcc996d3a9dc0b646e499ac7))
* **app-detector:** improve Rust and Python detection accuracy ([757268a](https://github.com/xDelph/devcli/commit/757268ad1b76758745c5255243bb0f3e796259f4))
* **auto-add:** discover apps in monorepo container dirs ([045dd31](https://github.com/xDelph/devcli/commit/045dd3109ea3583d9bc6e550dafb8b482e531186))
* **ci:** reap killed children in monitor tests, gate release-please on CI ([03dd586](https://github.com/xDelph/devcli/commit/03dd586df586dc2d7bf28f34ad8a5f9f4bbfe264))
* **ci:** stop the Test job from hanging on a real pm-daemon ([0310881](https://github.com/xDelph/devcli/commit/0310881134a7a8597f44cb21f205e82776b7ad83))
* **config:** preserve tilde paths on save and format migrated files ([3b7de1c](https://github.com/xDelph/devcli/commit/3b7de1cc74a6cacee3b7920d84998d0d6c66682e))
* **config:** tilde expansion, single-pass ambiguous resolution, pixel name collision ([61380ae](https://github.com/xDelph/devcli/commit/61380aef2a61080ba764f18ef489a57f763b30a2))
* **devcli-core:** harden command parsing and dependency dedup ([86214c7](https://github.com/xDelph/devcli/commit/86214c78e745a4a5d14ac6981616f0c4116effbf))
* **env-flow:** fix mono env load ([2083def](https://github.com/xDelph/devcli/commit/2083defb81382455fcb78779bcd5aceb53bf6232))
* filter status command to show only apps from current config ([3c4ce6c](https://github.com/xDelph/devcli/commit/3c4ce6c7679324b64ce7114f0d0698f43a86246c))
* fix log printing outside of TUI ([f4b5bf8](https://github.com/xDelph/devcli/commit/f4b5bf8f9efcab307fea6227bde47949d27d9e85))
* fix website responsivness ([c5fb69e](https://github.com/xDelph/devcli/commit/c5fb69e026786aa12cc0dbb3e4252248db4b4ca9))
* improve daemon process detection in spawner ([b012418](https://github.com/xDelph/devcli/commit/b0124182492d5f9aedd1799ee3aeaacc7ff17be7))
* isolate devcli state under devcli_CONFIG_DIR, un-ignore process tests, split CI caches ([2e4ce65](https://github.com/xDelph/devcli/commit/2e4ce65cc67147a8895b801a904afca3e67532a3))
* **log-viewer:** exact wrapped-row total so the last log rows are reachable ([1f1228d](https://github.com/xDelph/devcli/commit/1f1228dcaf76557f200655a5ee8d7ffc32ac0471))
* **log-viewer:** standard anchored scrolling in log panel ([bfc64d9](https://github.com/xDelph/devcli/commit/bfc64d91a0489bc407429c599a6f2dde2cc91a91))
* make stop and restart commands recognize alternative names ([3aa0d9b](https://github.com/xDelph/devcli/commit/3aa0d9b66aa3e02e4449cc8e39b3edd5a6a31a0a))
* prevent duplicate app starts when app is also a dependency ([dc905af](https://github.com/xDelph/devcli/commit/dc905af23a2d20a6251b2dc7dab025a2fd82d169))
* prevent spawner from stopping when parent exits (Ctrl+C) ([7a9b870](https://github.com/xDelph/devcli/commit/7a9b87077714d0cb93b8035efd33323bca5d3b80))
* **process-manager:** drop waitpid from the production kill path ([f888784](https://github.com/xDelph/devcli/commit/f888784e01f08ba9b992d4f6f58ddc502c70e9a9))
* **process-manager:** make process termination reliable on Linux ([f668ee9](https://github.com/xDelph/devcli/commit/f668ee952bac89fd413981ae442a263d61eb9cf7))
* **process-manager:** restore wrapper-process liveness without the PGID false positive ([393edb9](https://github.com/xDelph/devcli/commit/393edb966bd0a724dbfc4ef0516256cd141363a9))
* **process-manager:** stop reaping from is_running ([e81ec41](https://github.com/xDelph/devcli/commit/e81ec41fef0dcdf5a14e2bd52d21d78075bffc71))
* release links ([4b97c03](https://github.com/xDelph/devcli/commit/4b97c0371d39e6ebe350dfef598dc87f3b1335b5))
* **release:** configure release-please for a virtual Cargo workspace ([2fca0ff](https://github.com/xDelph/devcli/commit/2fca0ff9192bc4213fa07b63de144e9e1c0b9910))
* resolve clippy warnings in app-detector and env-flow ([ba700ef](https://github.com/xDelph/devcli/commit/ba700ef232a5ae4c1e3e9f58f2192db389cfa59b))
* script verify and install ([edfbb3d](https://github.com/xDelph/devcli/commit/edfbb3de5e132ffde7b1eec5bfd0890987807ff9))
* **tui:** capture mouse wheel and keep selection always visible in log viewer ([b0898ec](https://github.com/xDelph/devcli/commit/b0898ec1b98a32eb69b43890d0bb5a3be013907a))
* **tui:** pager-like scrolling without jumps; faster wheel in log viewer ([a42da78](https://github.com/xDelph/devcli/commit/a42da7863dbb17de731e65a0f6b5353cfba75026))
* **tui:** visible selection highlight and debounced mouse wheel in log viewer ([bbfbf3b](https://github.com/xDelph/devcli/commit/bbfbf3bd89eeb23dbf551d6881cd731db594ad52))
* **website:** update footer year, doc status, and active link logic ([edcb7ba](https://github.com/xDelph/devcli/commit/edcb7ba55b451162795b6a2079b903d981270c15))


### Performance Improvements

* **website:** optimize images using astro:assets ([5f96175](https://github.com/xDelph/devcli/commit/5f9617503b1e1e0a24d1edfafa58d45ef4188524))

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

## [0.1.0] - 2024-02-09

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

[Unreleased]: https://github.com/xDelph/devcli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/xDelph/devcli/releases/tag/v0.1.0
