# Implementation Plan

- [x] 1. Set up TUI infrastructure and core app

  - Add ratatui, crossterm, and syntect dependencies to Cargo.toml
  - Create tui module structure (app.rs, state.rs, theme.rs) in rustycli-core/src/tui/
  - Implement TuiApp with terminal initialization, event loop, and cleanup
  - Create AppState with config/process integration and Theme with GitUI colors
  - Add ui command entry point in CLI (rustycli-core/src/commands/ui.rs)
  - Add comprehensive inline comments explaining all structs, functions, and logic (like existing code)
  - Write unit tests in rustycli-core/src/tui/tests.rs for state management and theme
  - Ensure no compiler warnings (fix all unused variables, imports, dead code)
  - _Requirements: 1.1, 1.2, 1.4, 3.4_

- [x] 2. Implement main view with split-panel layout

  - Create MainView in rustycli-core/src/tui/views/main_view.rs with tab system (Status, Commands, Logs)
  - Implement split-panel layout (30% left app list, 70% right details)
  - Render project/app tree with expand/collapse and status indicators (●/○)
  - Add keyboard navigation (arrows/jk, Tab for panel switch, 1-3 for tabs, Space for expand)
  - Add inline comments explaining rendering logic, layout calculations, and event handling
  - Write unit tests in rustycli-core/src/tui/views/tests.rs for navigation state and selection logic
  - Ensure no compiler warnings (handle all match arms, use #[allow(dead_code)] only when necessary)
  - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.2, 3.4, 7.1, 7.2, 7.4, 7.5_

- [x] 3. Implement Status tab with app details

  - Render app details in right panel (name, type, status, PID, uptime, path, dependencies)
  - Implement background status polling (every 2 seconds) using tokio
  - Update UI when process states change
  - Add inline comments explaining polling logic and state updates
  - Write unit tests in rustycli-core/src/tui/views/tests.rs for status formatting and uptime calculations
  - Ensure no compiler warnings (handle async properly, no unused Results)
  - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.2, 3.3_

- [x] 4. Implement Commands tab with execution

  - Render command list grouped by environment (local, docker, k8s)
  - Create command execution popup in rustycli-core/src/tui/widgets/command_popup.rs with confirmation
  - Integrate with existing start_command to execute in background
  - Show execution feedback and update app status
  - Add inline comments explaining command parsing and execution flow
  - Write unit tests in rustycli-core/src/tui/widgets/tests.rs for command grouping and popup state
  - Ensure no compiler warnings (handle all error cases, no unwrap() in production code)
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [x] 5. Implement Logs tab with file browser

  - Create LogManager in rustycli-core/src/tui/log_manager.rs to discover log files in ~/.rustycli/logs/
  - Render log file list with name, size, and date
  - Handle empty state with helpful message
  - Add navigation to select log files
  - Add inline comments explaining file discovery, parsing, and sorting logic
  - Write unit tests in rustycli-core/src/tui/tests.rs for log file parsing and sorting
  - Ensure no compiler warnings (handle file I/O errors, no unwrap() on Results)
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [x] 6. Implement full-screen log viewer with beautification

  - Create LogViewerView in rustycli-core/src/tui/views/log_viewer.rs with full-screen layout
  - Implement log reading with lazy loading for large files
  - Add scrolling navigation (arrows/jk, PgUp/PgDn, Home/End, g/G)
  - Detect and prettify JSON with syntax highlighting using syntect
  - Implement search mode (/ to enter, n/N for next/prev match)
  - Add inline comments explaining lazy loading, JSON detection, and search algorithm
  - Write unit tests in rustycli-core/src/tui/views/tests.rs for JSON detection, scrolling bounds, and search
  - Ensure no compiler warnings (handle parse errors, bounds checking, no panics)
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 7.1_

- [x] 7. Add error handling and help overlay

  - Create error display system (status bar for non-blocking, modal for blocking)
  - Handle config and log reading errors gracefully
  - Implement help overlay in rustycli-core/src/tui/widgets/help_overlay.rs (? key) showing all keyboard shortcuts
  - Add inline comments explaining error propagation and display logic
  - Write unit tests in rustycli-core/src/tui/widgets/tests.rs for error formatting and help text
  - Ensure no compiler warnings (use proper error types, no string errors)
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 7.4_

- [ ]\* 8. Polish and optimize
  - Add loading indicators for async operations
  - Handle terminal resize events
  - Optimize rendering performance with dirty flags
  - Fine-tune colors, spacing, and visual consistency
  - Add inline comments for optimization techniques
  - Write performance tests if needed
  - Final pass to eliminate all compiler warnings
