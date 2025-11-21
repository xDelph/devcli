# Implementation Plan

- [x] 1. Add stage support to configuration models

  - Add `Stage` enum with validation methods to `config/models.rs`
  - Add optional `stage` field to `App` struct with proper serde attributes
  - Add `stage` field to `ProcessInfo` struct in `process/tracker.rs`
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 2. Extend environment file detection for stage-specific files

  - Update `find_env_file()` in `detection/environments/orbstack.rs` to accept stage parameter and implement priority logic
  - Update `load_env_vars_for_runtime()` in `detection/environments/orbstack.rs` to accept stage parameter
  - Create `detection/environments/orbstack_stage_test.rs` for stage-specific file detection tests
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

- [x] 3. Integrate stage into start command execution

  - Add `stage` field to `StartCommandArgs` in `commands/start/resolver.rs`
  - Update `start_single_app_process()` in `commands/start/executor.rs` to accept and use stage parameter
  - Add stage validation and effective stage determination logic
  - Update env file detection calls to pass stage information
  - Add logging to show which env file is being used
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 6.1, 6.2, 6.5_

- [x] 4. Add stage management to config commands

  - Create `config_set_stage()` function in `commands/config/edit.rs`
  - Update `config_list()` in `commands/config/list.rs` to display stage and env file information
  - Wire up new command in CLI argument parser
  - Create `commands/config/stage_test.rs` for stage configuration tests
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 5.3, 5.4, 5.5_

- [x] 5. Add stage detection to auto-add command

  - Create `detect_stage_files()` helper function in `commands/auto_add/interactive.rs`
  - Add stage prompting logic to interactive flow
  - Update app creation to include stage field
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_

- [x] 6. Update TUI to display stage information

  - Add stage indicator to app list rendering in `tui/views/main_view/renderer.rs`
  - Update process info display to show stage
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

- [x] 7. Validate backward compatibility and error handling
  - Run existing tests to ensure no regressions
  - Test loading configs without stage field
  - Test invalid stage values and missing files
  - Verify error messages are clear and helpful
  - Run `cargo build`, `cargo clippy`, and `cargo test` to ensure all pass
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 9.1, 9.2, 9.3, 9.4, 9.5_
