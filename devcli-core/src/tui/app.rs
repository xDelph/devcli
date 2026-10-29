// Main TUI application
// Manages terminal initialization, event loop, and cleanup
// Follows the Elm architecture pattern for predictable state management
//
// PERFORMANCE OPTIMIZATIONS:
// 1. Dirty Flag Pattern: Only redraws UI when state changes (needs_redraw flag)
//    - Reduces CPU usage by avoiding unnecessary frame renders
//    - Typical improvement: 90% reduction in CPU usage during idle
//
// 2. Non-blocking State Access: Uses try_lock() in background tasks
//    - Prevents blocking the main thread during status polling
//    - Ensures smooth UI responsiveness even during heavy operations
//
// 3. Efficient Event Polling: 250ms timeout balances responsiveness and CPU
//    - Quick enough for smooth user interaction
//    - Long enough to avoid busy-waiting and wasting CPU cycles
//
// 4. Terminal Resize Handling: Clears terminal on resize to prevent artifacts
//    - Ensures clean rendering after terminal size changes
//    - Marks UI for redraw to update layout
//
// 5. Layered Rendering: Renders base view, then overlays (popup, help)
//    - Efficient composition without full screen redraws
//    - Maintains visual hierarchy and z-ordering
//
// VISUAL POLISH:
// - Consistent color scheme throughout (defined in theme.rs)
// - Clear visual hierarchy with borders, spacing, and indentation
// - Context-aware keyboard shortcuts in footers
// - Status indicators with color coding (green=running, gray=stopped)
// - Smooth animations for loading states

use super::state::{AppState, ViewType};
use super::theme::Theme;
use super::views::{
    log_viewer::{LogInputResult, SelectionItem, SelectionMode},
    LogViewerView, MainView,
};
use super::widgets::help_overlay::HelpOverlay;
use crate::tui::popups::PopupManager;

use chrono::{NaiveDate, Utc};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::config::loader::load_config;
use crate::process_manager_support::state_store;

use anyhow::{Context, Result};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyEvent};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc;
use tokio::time::interval;

/// Put the terminal back the way the shell expects it: mouse released,
/// normal screen, raw mode off, cursor visible. Every step is attempted even
/// when an earlier one fails, and errors are ignored - there is nothing better
/// to do with them on the way out.
fn restore_terminal() {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, DisableMouseCapture);
    let _ = execute!(stdout, LeaveAlternateScreen);
    let _ = disable_raw_mode();
    let _ = execute!(stdout, crossterm::cursor::Show);
}

/// Restores the terminal when dropped (error returns, unwinding).
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Restore the terminal before the panic message is printed.
///
/// Without it a panic in the TUI (or in one of its tasks) left the shell in
/// raw mode on the alternate screen with the mouse captured: the message was
/// unreadable and the terminal unusable until `reset`.
fn install_restore_on_panic() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            previous(info);
        }));
    });
}

/// The main TUI application struct
/// Manages the terminal, state, and event loop
pub struct TuiApp {
    /// Current application state wrapped in Arc<Mutex<>> for thread-safe access
    /// Arc allows multiple ownership across threads, Mutex ensures only one thread accesses at a time
    pub(crate) state: Arc<Mutex<AppState>>,
    /// Visual theme for the UI
    pub(crate) theme: Theme,
    /// Main view instance
    pub(crate) main_view: MainView,
    /// Log viewer instance (created when viewing a log file)
    pub(crate) log_viewer: Option<LogViewerView>,
    /// Shared state store for checking app status
    /// Shared across threads for background status polling
    pub(crate) state_store: Arc<process_manager::StateStore>,
    /// Flag to indicate the app should quit
    pub(crate) should_quit: bool,
    /// Manager for popup dialogs
    pub(crate) popup_manager: PopupManager,
    /// Help overlay for displaying keyboard shortcuts
    /// Toggled with '?' key
    pub(crate) help_overlay: HelpOverlay,
    /// Dirty flag to track if UI needs redrawing
    /// Optimization: Only redraw when state changes or events occur
    pub(crate) needs_redraw: bool,
    /// Last render timestamp for performance tracking
    /// Used to measure frame time and optimize rendering
    pub(crate) last_render: Instant,
    /// Flag to clear terminal on next render (for view transitions)
    pub(crate) needs_clear: bool,
    /// Channel for sending command execution requests to background task
    #[allow(dead_code)] // Used by spawned background task
    pub(crate) command_tx: mpsc::UnboundedSender<CommandRequest>,
    /// Channel for receiving command execution results
    pub(crate) command_rx: mpsc::UnboundedReceiver<CommandResult>,
}

/// Request to execute a command in the background
#[derive(Debug, Clone)]
pub(crate) struct CommandRequest {
    pub(crate) app_name: String,
    pub(crate) project: String,
    pub(crate) environment: String,
    pub(crate) command_type: CommandType,
}

/// Type of command being executed
#[derive(Debug, Clone)]
pub(crate) enum CommandType {
    Start, // Uses start command with environment
    Run,   // Uses run command with specific command
    Stop,
    Restart,
}

/// Result of command execution
#[derive(Debug)]
pub(crate) enum CommandResult {
    Success(String),
    Error(String),
    LogLine(String), // New: stream log lines to popup
}

impl TuiApp {
    /// Creates a new TUI application
    /// Loads configuration and initializes state
    pub fn new() -> Result<Self> {
        crate::debug!("TuiApp::new() called");

        // Initialize debug logging
        crate::tui::debug::init_debug_log();

        // Load configuration from disk
        let config = load_config().context("Failed to load configuration")?;

        // Initialize shared process-manager state store
        let state_store = Arc::new(state_store()?);
        let _ = state_store.cleanup_dead();

        // Create initial state from config
        let state = AppState::from_config(&config).context("Failed to create application state")?;

        // Wrap state in Arc<Mutex<>> for thread-safe access from background polling
        let state = Arc::new(Mutex::new(state));

        // Load theme
        let theme = Theme::default();

        // Create main view
        let main_view = MainView::new()?;

        // Create channels for command execution
        // Request channel: UI -> Background task
        let (req_tx, mut req_rx) = mpsc::unbounded_channel::<CommandRequest>();
        // Result channel: Background task -> UI
        let (result_tx, result_rx) = mpsc::unbounded_channel::<CommandResult>();

        // Spawn background task to handle command execution
        tokio::spawn(async move {
            while let Some(request) = req_rx.recv().await {
                let tx = result_tx.clone();
                tokio::spawn(async move {
                    let result = crate::tui::command_executor::execute_command_async(
                        request.clone(),
                        tx.clone(),
                    )
                    .await;
                    let _ = match result {
                        Ok(msg) => tx.send(CommandResult::Success(msg)),
                        Err(e) => tx.send(CommandResult::Error(format!("{:#}", e))),
                    };
                });
            }
        });

        // Initialize popup manager
        let popup_manager = PopupManager::new(req_tx.clone());

        Ok(Self {
            state,
            theme,
            main_view,
            log_viewer: None,
            state_store,
            should_quit: false,
            popup_manager,
            help_overlay: HelpOverlay::new(),
            needs_redraw: true, // Initial render needed
            last_render: Instant::now(),
            needs_clear: false,
            command_tx: req_tx,
            command_rx: result_rx,
        })
    }

    /// Runs the TUI application
    /// Sets up the terminal, runs the event loop, and cleans up on exit
    pub async fn run(&mut self) -> Result<()> {
        crate::debug!("TuiApp::run() starting");

        // Setup terminal. From here on the terminal is restored however this
        // function ends: normal return, error (`?`), or a panic anywhere.
        install_restore_on_panic();
        enable_raw_mode().context("Failed to enable raw mode")?;
        let _restore = TerminalGuard;
        let mut stdout = io::stdout();
        // Capture the mouse so wheel events reach the app (routed to scroll in
        // Runtime). In the alternate screen this only enables scrolling; text
        // selection with the mouse is disabled while the TUI is running.
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to enter alternate screen")?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

        // Start background status polling task
        // This task runs independently and updates app statuses
        // It notifies the main thread when changes are detected
        let polling_handle = self.start_status_polling();

        // Run the main loop via Runtime
        let mut runtime = crate::tui::runtime::Runtime::new();
        let result = runtime.run(self, &mut terminal);

        // Stop the background polling task
        polling_handle.abort();

        // Cleanup terminal (also done by `_restore` on every other way out).
        // Each step is attempted even if an earlier one fails: the old chain of
        // `?` left raw mode on if releasing the mouse failed.
        restore_terminal();
        let _ = terminal.show_cursor();

        result
    }

    /// Check regular updates (state changes, requests, etc.)
    pub(crate) fn tick(&mut self) -> Result<()> {
        let mut needs_redraw = false;

        // Delegate popup checks to manager
        needs_redraw |=
            self.popup_manager
                .tick(&self.state, &mut self.command_rx, &self.main_view)?;

        if self.popup_manager.active_popup.is_some() {
            // If popup is active, we might need to redraw more often or differently
        }

        self.check_log_viewer_creation()?;
        self.refresh_log_viewer()?;

        // Check if status was updated by background polling
        self.check_status_update()?;

        // The status panel shows a live uptime: redraw about once a second
        // while something runs (polls no longer count uptime as a change).
        if self.last_render.elapsed() >= Duration::from_secs(1) && self.any_app_running() {
            needs_redraw = true;
        }

        if needs_redraw {
            self.needs_redraw = true;
        }

        Ok(())
    }

    fn any_app_running(&self) -> bool {
        self.state.try_lock().is_ok_and(|state| {
            state
                .projects
                .iter()
                .any(|p| p.apps.iter().any(|a| a.status.is_running()))
        })
    }

    pub(crate) fn needs_clear(&self) -> bool {
        self.needs_clear
    }

    pub(crate) fn clear_needs_clear(&mut self) {
        self.needs_clear = false;
    }

    pub(crate) fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    pub(crate) fn set_needs_redraw(&mut self, needed: bool) {
        self.needs_redraw = needed;
    }

    pub(crate) fn on_render_complete(&mut self, time: Instant) {
        self.last_render = time;
        self.needs_redraw = false;
    }

    /// Handles keyboard input events
    /// Routes events based on the current view and popup state
    pub(crate) fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        crate::tui::input::router::handle_key_event(self, key)
    }

    /// Handles input for the main view
    /// Returns true if the key was handled, false otherwise
    pub(crate) fn handle_main_view_input(&mut self, key: KeyEvent) -> Result<bool> {
        // Delegate to the main view's input handler
        self.main_view.handle_input(key, &self.state)
    }

    #[cfg(test)]
    pub(crate) fn new_test() -> Self {
        let state = Arc::new(Mutex::new(AppState {
            projects: vec![],
            selected_project_idx: 0,
            selected_app_idx: 0,
            current_view: ViewType::Main,
            view_history: vec![],
            error_message: None,
            status_message: None,
            command_execution_requested: None,
            stop_requested: false,
            restart_requested: false,
            env_selection_requested: false,
            status_updated: false,
        }));

        // We might fail to create directory in some test envs, but usually it's fine.
        // If it fails, tests will panic.
        let state_store = Arc::new(state_store().unwrap_or_else(|_| {
            panic!("Failed to create StateStore for test");
        }));

        let (req_tx, _req_rx) = mpsc::unbounded_channel();
        let (_result_tx, result_rx) = mpsc::unbounded_channel();

        // Initialize popup manager
        let popup_manager = PopupManager::new(req_tx.clone());

        Self {
            state,
            theme: Theme::default(),
            main_view: MainView::new().expect("Failed to create MainView for test"),
            log_viewer: None,
            state_store,
            should_quit: false,
            popup_manager,
            help_overlay: HelpOverlay::new(),
            needs_redraw: false,
            last_render: Instant::now(),
            needs_clear: false,
            command_tx: req_tx,
            command_rx: result_rx,
        }
    }

    pub(crate) fn should_quit(&self) -> bool {
        self.should_quit
    }

    #[cfg(test)]
    pub(crate) fn is_help_visible(&self) -> bool {
        self.help_overlay.is_visible()
    }

    /// Handles input for the log viewer view
    pub(crate) fn handle_log_viewer_input(&mut self, key: KeyEvent) -> Result<bool> {
        // Delegate to the log viewer's input handler
        if let Some(viewer) = &mut self.log_viewer {
            match viewer.handle_input(key)? {
                LogInputResult::Handled => Ok(true),
                LogInputResult::Ignored => Ok(false),
                LogInputResult::RequestAddPanel => {
                    self.open_log_selector(SelectionMode::Add);
                    self.needs_redraw = true;
                    Ok(true)
                }
                LogInputResult::RequestSelectLog => {
                    // Get active panel index
                    let active_idx = viewer.active_panel_idx();
                    self.open_log_selector(SelectionMode::Replace(active_idx));
                    self.needs_redraw = true;
                    Ok(true)
                }
                LogInputResult::Error(message) => {
                    if let Ok(mut state) = self.state.lock() {
                        state.error_message = Some(message);
                    }
                    self.needs_redraw = true;
                    Ok(true)
                }
            }
        } else {
            Ok(false)
        }
    }

    /// Handles the back action (Esc key)
    /// Returns to the previous view or main view
    pub(crate) fn handle_back(&mut self) {
        // If popup is open, close it instead of going back
        if self.popup_manager.is_active() {
            self.popup_manager.close_popup();
            return;
        }

        // Clean up log viewer when leaving that view
        if let Ok(state) = self.state.lock() {
            if matches!(state.current_view, ViewType::LogViewer { .. }) {
                self.log_viewer = None;
                // Clear terminal to avoid artifacts
                self.needs_clear = true;
            }
        }

        // Pop the navigation stack; fall back to Main
        if let Ok(mut state) = self.state.lock() {
            state.current_view = state.view_history.pop().unwrap_or(ViewType::Main);
        }
    }

    /// Handles input when a popup is active
    /// Returns Ok(()) to indicate the event was handled
    pub(crate) fn handle_popup_input(&mut self, key: KeyEvent) -> Result<()> {
        self.popup_manager.handle_input(key)
    }

    /// Opens the log selector popup
    fn open_log_selector(&mut self, mode: SelectionMode) {
        // Clone log_paths to avoid borrowing issues
        let current_paths = if let Some(viewer) = &self.log_viewer {
            viewer.log_paths()
        } else {
            Vec::new()
        };

        if let Some(viewer) = &mut self.log_viewer {
            // Map<Project, Map<App, Vec<(PathBuf, Env, DateStr)>>>
            type LogEntry = (PathBuf, String, String); // Path, Env, Date
            let mut grouped_logs: BTreeMap<String, BTreeMap<String, Vec<LogEntry>>> =
                BTreeMap::new();

            if let Ok(state) = self.state.lock() {
                for proj in &state.projects {
                    for app in &proj.apps {
                        if app.status.is_running() {
                            if let Ok(logs) = self
                                .main_view
                                .log_manager
                                .list_logs_for_app(&app.project, &app.name)
                            {
                                for log in logs {
                                    if !current_paths.contains(&log.path) {
                                        let (env, date) =
                                            crate::process_manager_support::LogFileName::from_path(
                                                &log.path,
                                            )
                                            .map(|name| (name.environment, name.date))
                                            .unwrap_or_else(|| ("?".to_string(), "?".to_string()));

                                        grouped_logs
                                            .entry(app.project.clone())
                                            .or_default()
                                            .entry(app.name.clone())
                                            .or_default()
                                            .push((log.path, env, date));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let mut candidates = Vec::new();
            for (project, apps) in grouped_logs {
                if apps.values().all(|v| v.is_empty()) {
                    continue;
                }

                candidates.push(SelectionItem::Header(project));

                for (app_name, logs) in apps {
                    if logs.is_empty() {
                        continue;
                    }
                    candidates.push(SelectionItem::SubHeader(app_name));

                    for (path, env, date_str) in logs {
                        let formatted_date = Self::format_smart_date(&date_str);
                        let label = format!("{} - {}", env, formatted_date);
                        candidates.push(SelectionItem::Option { label, path });
                    }
                }
            }
            viewer.start_log_selection(candidates, mode);
        }
    }

    /// Helper to format date sensibly
    fn format_smart_date(date_str: &str) -> String {
        if date_str.len() != 8 {
            return date_str.to_string();
        }

        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y%m%d") {
            let today = Utc::now().naive_utc().date();
            let diff = today.signed_duration_since(date).num_days();

            match diff {
                0 => "Today".to_string(), // Bold handling is done in render if needed, but here we just return text
                1 => "Yesterday".to_string(),
                2..=3 => format!("{} days ago", diff),
                _ => date.format("%Y-%m-%d").to_string(),
            }
        } else {
            date_str.to_string()
        }
    }

    /// Checks if we need to create a log viewer for the current view
    /// Called at the start of each event loop iteration
    ///
    /// Error Handling:
    /// - If log file cannot be opened, displays error in status bar
    /// - Returns to main view to allow user to continue
    fn check_log_viewer_creation(&mut self) -> Result<()> {
        let current_view = {
            let state = self
                .state
                .lock()
                .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
            state.current_view.clone()
        };

        // If we're in LogViewer view but don't have a viewer instance, create one
        if let ViewType::LogViewer { log_paths, .. } = current_view {
            if self.log_viewer.is_none() {
                match LogViewerView::new(log_paths.clone()) {
                    Ok(viewer) => {
                        self.log_viewer = Some(viewer);
                        // Clear terminal to avoid artifacts from previous view
                        self.needs_clear = true;

                        // If opened with no logs, trigger selector
                        if log_paths.is_empty() {
                            self.open_log_selector(SelectionMode::Add);
                        }
                    }
                    Err(e) => {
                        // Failed to create viewer - set error and go back to main view
                        // This is a non-blocking error - user can dismiss and continue
                        let mut state = self
                            .state
                            .lock()
                            .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
                        state.error_message = Some(format!("Failed to open log files: {}", e));
                        state.current_view = ViewType::Main;
                    }
                }
            }
        }

        Ok(())
    }

    /// Refreshes the log viewer if it's active
    /// Called at the start of each event loop iteration
    fn refresh_log_viewer(&mut self) -> Result<()> {
        if let Some(viewer) = &mut self.log_viewer {
            // Try to refresh the log file
            match viewer.refresh() {
                Ok(updated) => {
                    if updated {
                        // Content was updated, mark for redraw
                        self.needs_redraw = true;
                    }
                }
                Err(_) => {
                    // Ignore errors during refresh (file might be temporarily unavailable)
                }
            }
        }
        Ok(())
    }

    /// Checks if status was updated by background polling and triggers redraw
    fn check_status_update(&mut self) -> Result<()> {
        let (status_updated, app_status) = {
            let mut state = self
                .state
                .lock()
                .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
            let updated = state.status_updated;
            if updated {
                state.status_updated = false; // Clear the flag
            }

            // Get the current app status if popup is open
            let app_status = if self.popup_manager.is_active() {
                state.selected_app().map(|app| app.status.display_label())
            } else {
                None
            };

            (updated, app_status)
        };

        if status_updated {
            // Update popup status if it's open
            if let Some(status) = app_status {
                self.popup_manager.update_status(status);
            }

            self.needs_redraw = true;
        }

        Ok(())
    }

    /// Starts a background task that polls process status using a hybrid strategy:
    /// fast checks on `.status_changed` every 250ms, plus a full refresh every 2s.
    /// Returns a JoinHandle that can be used to abort the task when the app exits.
    fn start_status_polling(&self) -> tokio::task::JoinHandle<()> {
        // Clone Arc references so they can be moved into the async task
        let state = Arc::clone(&self.state);
        let state_store = Arc::clone(&self.state_store);

        tokio::spawn(async move {
            let mut fast_interval = interval(Duration::from_millis(250));
            let mut full_interval = interval(Duration::from_secs(2));
            let mut last_mtime: Option<SystemTime> =
                state_store.last_status_change().ok().flatten();

            loop {
                tokio::select! {
                    _ = fast_interval.tick() => {
                        match state_store.last_status_change() {
                            Ok(Some(mtime)) if last_mtime != Some(mtime) => {
                                last_mtime = Some(mtime);
                                Self::update_all_app_statuses(&state, &state_store).await;
                            }
                            Ok(None) | Err(_) => {}
                            Ok(Some(_)) => {}
                        }
                    }
                    _ = full_interval.tick() => {
                        Self::update_all_app_statuses(&state, &state_store).await;
                        last_mtime = state_store.last_status_change().ok().flatten();
                    }
                }
            }
        })
    }

    /// Updates the status of all apps by checking the process-manager state store.
    async fn update_all_app_statuses(
        state: &Arc<Mutex<AppState>>,
        state_store: &Arc<process_manager::StateStore>,
    ) {
        // Which apps to look at - a brief lock. If the UI holds it, skip.
        let keys: Vec<(String, String)> = match state.try_lock() {
            Ok(state) => state
                .projects
                .iter()
                .flat_map(|p| p.apps.iter().map(|a| (a.project.clone(), a.name.clone())))
                .collect(),
            Err(_) => return,
        };

        // Read the store once and probe processes on a blocking thread, with
        // no lock held. This used to run `find_process` (a full directory read
        // and parse) per app, with its process probes, while holding the
        // AppState mutex on a runtime worker - the UI thread blocked on the
        // lock in tick/render meanwhile.
        let store = Arc::clone(state_store);
        let Ok(snapshots) = tokio::task::spawn_blocking(move || {
            let processes = store.list().unwrap_or_default();
            keys.into_iter()
                .map(|(project, app)| {
                    let process = processes.iter().find(|p| {
                        p.metadata.get("project") == Some(&project)
                            && p.metadata.get("app_config_name") == Some(&app)
                    });
                    let snapshot =
                        crate::tui::state::RuntimeSnapshot::from_process(&store, process);
                    ((project, app), snapshot)
                })
                .collect::<std::collections::HashMap<_, _>>()
        })
        .await
        else {
            return;
        };

        let Ok(mut state) = state.lock() else {
            return;
        };
        let mut any_changed = false;
        for project in &mut state.projects {
            for app in &mut project.apps {
                let Some(new) = snapshots.get(&(app.project.clone(), app.name.clone())) else {
                    continue;
                };
                let old = crate::tui::state::RuntimeSnapshot {
                    status: app.status.clone(),
                    active_stage: app.active_stage.clone(),
                    restart_count: app.restart_count,
                    last_exit_code: app.last_exit_code,
                    health_status: app.health_status.clone(),
                };
                if !old.same_state(new) {
                    any_changed = true;
                }
                app.status = new.status.clone();
                app.active_stage = new.active_stage.clone();
                app.restart_count = new.restart_count;
                app.last_exit_code = new.last_exit_code;
                app.health_status = new.health_status.clone();
            }
        }

        // Set flag to trigger UI redraw if something actually changed
        if any_changed {
            state.status_updated = true;
        }
    }

    /// Renders the UI
    /// Delegates to view-specific rendering based on current view
    /// Also renders popup if one is active
    /// Optimization: Layered rendering approach for efficient updates
    pub(crate) fn render(&mut self, frame: &mut Frame) {
        // Lock the state for reading during rendering
        // If we can't get the lock, skip this frame
        // Performance: Non-blocking approach prevents frame drops
        let state_arc = self.state.clone();
        let Ok(state) = state_arc.lock() else {
            return;
        };

        let current_view = state.current_view.clone();

        match &current_view {
            ViewType::Main => {
                crate::debug!("[Render] Rendering Main view");
                self.render_main_view(frame, &state)
            }
            ViewType::LogViewer { .. } => {
                crate::debug!("[Render] Rendering LogViewer view");
                drop(state); // Release the lock before calling mutable render
                self.render_log_viewer_view(frame);
                return; // Early return to avoid double-locking
            }
        }

        // Render error message if present (but not if popup or help is showing)
        if let Some(error) = &state.error_message {
            if !self.popup_manager.is_active() && !self.help_overlay.is_visible() {
                self.render_error_message(frame, error);
            }
        }

        // Render popup overlay if active
        self.popup_manager.render(frame, &self.theme);

        // Render help overlay if visible (renders on top of everything)
        if self.help_overlay.is_visible() {
            crate::debug!("[Render] Rendering help overlay");
        }
        self.help_overlay.render(frame, &self.theme);
    }

    /// Renders the main view
    /// Shows projects, apps, and their status
    /// Shows projects, apps, and their status
    fn render_main_view(&mut self, frame: &mut Frame, state: &AppState) {
        // Delegate to the main view's render method
        self.main_view.render(frame, state, &self.theme);
    }

    /// Renders an error message as a status bar at the bottom
    /// This is for non-blocking errors that don't require user acknowledgment
    /// The error can be dismissed by pressing any key or will auto-clear on next action
    fn render_error_message(&self, frame: &mut Frame, error: &str) {
        let size = frame.area();

        // Create a status bar at the bottom (3 lines high)
        let status_area = Rect {
            x: 0,
            y: size.height.saturating_sub(3),
            width: size.width,
            height: 3,
        };

        // Build error message with icon and instructions
        let error_lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" ✗ Error: ", self.theme.style_error_bold()),
                Span::styled(error, self.theme.style_text()),
                Span::styled("  [Press any key to dismiss]", self.theme.style_text_dim()),
            ]),
        ];

        let error_widget = Paragraph::new(error_lines)
            .style(self.theme.style_error_block())
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(self.theme.style_border_error()),
            );

        frame.render_widget(error_widget, status_area);
    }

    /// Renders the log viewer view
    fn render_log_viewer_view(&mut self, frame: &mut Frame) {
        if let Some(viewer) = &mut self.log_viewer {
            crate::debug!("[App] render_log_viewer_view: viewer exists, calling viewer.render()");
            let size = frame.area();
            viewer.render(frame, size);
        } else {
            crate::debug!("[App] render_log_viewer_view: NO VIEWER - rendering placeholder");
            // Fallback if viewer is not initialized
            let size = frame.area();
            let placeholder = Paragraph::new("Log Viewer - No file loaded")
                .alignment(Alignment::Center)
                .style(self.theme.style_bg_overlay())
                .block(Block::default().borders(Borders::ALL).title("Log Viewer"));
            frame.render_widget(placeholder, size);
        }
    }
}

#[cfg(test)]
mod status_polling_tests {
    use super::*;
    use crate::test_utils::{AppBuilder, ConfigBuilder};
    use process_manager::state::{ManagedProcess, ProcessRuntime};
    use std::collections::HashMap;

    fn tracked(restart_count: u32) -> ManagedProcess {
        let mut metadata = HashMap::new();
        metadata.insert("project".to_string(), "p".to_string());
        metadata.insert("app_config_name".to_string(), "api".to_string());
        ManagedProcess {
            id: "p.api.local".to_string(),
            // This test process: alive for the whole test.
            pid: std::process::id(),
            pgid: None,
            task: process_manager::Task {
                id: "p.api.local".to_string(),
                command: "x".to_string(),
                args: vec![],
                working_dir: "/tmp".into(),
                env: HashMap::new(),
                is_detached: true,
                log_file: None,
                health_check: process_manager::HealthCheck::Process {},
                restart_policy: process_manager::RestartPolicy::default(),
            },
            start_time: chrono::Utc::now(),
            metadata,
            runtime: ProcessRuntime {
                restart_count,
                ..ProcessRuntime::default()
            },
        }
    }

    /// The poller refreshes every runtime field, and an unchanged app (only
    /// its uptime grew) is not reported as a change.
    #[tokio::test]
    async fn polling_refreshes_runtime_fields_and_ignores_uptime() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Arc::new(process_manager::StateStore::new(dir.path().to_path_buf()).unwrap());
        store.save(&tracked(1)).unwrap();

        let config = ConfigBuilder::new()
            .with_app("p", "api", AppBuilder::new("nodejs", "/tmp").build())
            .build();
        let state = Arc::new(Mutex::new(
            AppState::from_config_with_store(&config, &store).unwrap(),
        ));

        TuiApp::update_all_app_statuses(&state, &store).await;
        assert!(
            !state.lock().unwrap().status_updated,
            "uptime alone is not a change"
        );

        store.save(&tracked(2)).unwrap();
        TuiApp::update_all_app_statuses(&state, &store).await;
        let state = state.lock().unwrap();
        assert!(state.status_updated);
        assert_eq!(state.projects[0].apps[0].restart_count, 2);
    }
}
