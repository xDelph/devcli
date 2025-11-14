// Main TUI application
// Manages terminal initialization, event loop, and cleanup
// Follows the Elm architecture pattern for predictable state management

use super::state::{AppState, ViewType};
use super::theme::Theme;
use super::views::MainView;
use crate::config::loader::load_config;
use crate::process::tracker::ProcessTracker;
use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::interval;

/// The main TUI application struct
/// Manages the terminal, state, and event loop
pub struct TuiApp {
    /// Current application state wrapped in Arc<Mutex<>> for thread-safe access
    /// Arc allows multiple ownership across threads, Mutex ensures only one thread accesses at a time
    state: Arc<Mutex<AppState>>,
    /// Visual theme for the UI
    theme: Theme,
    /// Main view instance
    main_view: MainView,
    /// Process tracker for checking app status
    /// Shared across threads for background status polling
    process_tracker: Arc<ProcessTracker>,
    /// Flag to indicate the app should quit
    should_quit: bool,
}

impl TuiApp {
    /// Creates a new TUI application
    /// Loads configuration and initializes state
    pub fn new() -> Result<Self> {
        // Load configuration from disk
        let config = load_config().context("Failed to load configuration")?;
        
        // Initialize process tracker wrapped in Arc for shared ownership
        let process_tracker = Arc::new(ProcessTracker::new()?);
        
        // Create initial state from config
        let state = AppState::from_config(&config, &process_tracker)
            .context("Failed to create application state")?;
        
        // Wrap state in Arc<Mutex<>> for thread-safe access from background polling
        let state = Arc::new(Mutex::new(state));
        
        // Load theme
        let theme = Theme::default();
        
        // Create main view
        let main_view = MainView::new();

        Ok(Self {
            state,
            theme,
            main_view,
            process_tracker,
            should_quit: false,
        })
    }

    /// Runs the TUI application
    /// Sets up the terminal, runs the event loop, and cleans up on exit
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode().context("Failed to enable raw mode")?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to enter alternate screen")?;
        
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

        // Start background status polling task
        // This task runs independently and updates app statuses every 2 seconds
        let polling_handle = self.start_status_polling();

        // Run the main loop
        let result = self.run_event_loop(&mut terminal);

        // Stop the background polling task
        polling_handle.abort();

        // Cleanup terminal
        disable_raw_mode().context("Failed to disable raw mode")?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .context("Failed to leave alternate screen")?;
        terminal.show_cursor().context("Failed to show cursor")?;

        result
    }

    /// The main event loop
    /// Handles user input and renders the UI
    fn run_event_loop<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        while !self.should_quit {
            // Render the current state
            terminal.draw(|f| self.render(f))?;

            // Wait for an event with a timeout
            // This allows us to update the UI periodically even without user input
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key_event(key)?;
                }
            }
        }

        Ok(())
    }

    /// Handles keyboard input events
    /// Routes events based on the current view
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        // Global shortcuts that work in any view
        match key.code {
            // Quit the application
            KeyCode::Char('q') => {
                self.should_quit = true;
                return Ok(());
            }
            // Handle Ctrl+C to quit
            KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return Ok(());
            }
            // Go back to previous view
            KeyCode::Esc => {
                self.handle_back();
                return Ok(());
            }
            _ => {}
        }

        // Get current view type by locking state briefly
        let current_view = {
            let state = self.state.lock().expect("Failed to lock state");
            state.current_view.clone()
        };

        // View-specific handling
        match &current_view {
            ViewType::Main => self.handle_main_view_input(key),
            ViewType::CommandList { .. } => self.handle_command_list_input(key),
            ViewType::LogBrowser { .. } => self.handle_log_browser_input(key),
            ViewType::LogViewer { .. } => self.handle_log_viewer_input(key),
        }
    }

    /// Handles input for the main view
    fn handle_main_view_input(&mut self, key: KeyEvent) -> Result<()> {
        // Delegate to the main view's input handler
        self.main_view.handle_input(key, &mut self.state)?;
        Ok(())
    }

    /// Handles input for the command list view
    fn handle_command_list_input(&mut self, _key: KeyEvent) -> Result<()> {
        // TODO: Implement command list navigation and execution
        Ok(())
    }

    /// Handles input for the log browser view
    fn handle_log_browser_input(&mut self, _key: KeyEvent) -> Result<()> {
        // TODO: Implement log browser navigation
        Ok(())
    }

    /// Handles input for the log viewer view
    fn handle_log_viewer_input(&mut self, _key: KeyEvent) -> Result<()> {
        // TODO: Implement log viewer navigation and search
        Ok(())
    }

    /// Handles the back action (Esc key)
    /// Returns to the previous view or main view
    fn handle_back(&mut self) {
        // For now, always return to main view
        // TODO: Implement view stack for proper back navigation
        if let Ok(mut state) = self.state.lock() {
            state.current_view = ViewType::Main;
        }
    }

    /// Starts a background task that polls process status every 2 seconds
    /// This keeps the UI updated with current running states without blocking user interaction
    /// Returns a JoinHandle that can be used to abort the task when the app exits
    fn start_status_polling(&self) -> tokio::task::JoinHandle<()> {
        // Clone Arc references so they can be moved into the async task
        let state = Arc::clone(&self.state);
        let process_tracker = Arc::clone(&self.process_tracker);

        tokio::spawn(async move {
            // Create an interval that ticks every 2 seconds
            let mut interval = interval(Duration::from_secs(2));

            loop {
                // Wait for the next tick
                interval.tick().await;

                // Try to acquire the state lock
                // If we can't get it immediately, skip this update cycle
                // This prevents blocking if the main thread is using the state
                if let Ok(mut state) = state.try_lock() {
                    // Update status for all apps in all projects
                    for project in &mut state.projects {
                        for app in &mut project.apps {
                            // Check if the app is currently running
                            app.status = Self::check_app_status(&app.name, &process_tracker);
                        }
                    }
                }
            }
        })
    }

    /// Checks the current status of an app by querying the process tracker
    /// Returns the updated AppStatus (Running with details or Stopped)
    fn check_app_status(
        app_name: &str,
        process_tracker: &ProcessTracker,
    ) -> crate::tui::state::AppStatus {
        use crate::tui::state::AppStatus;

        // Try to get process info from the tracker
        match process_tracker.get_process(app_name) {
            Ok(Some(process_info)) => {
                // Verify the process is actually still running
                if process_tracker.is_running(process_info.pid) {
                    // Calculate uptime from start time to now
                    let uptime = chrono::Utc::now()
                        .signed_duration_since(process_info.start_time);
                    
                    AppStatus::Running {
                        pid: process_info.pid,
                        uptime,
                        start_time: process_info.start_time,
                    }
                } else {
                    // Process is in tracker but not running anymore
                    AppStatus::Stopped
                }
            }
            Ok(None) => {
                // No process info found - app is stopped
                AppStatus::Stopped
            }
            Err(_) => {
                // Error checking status - mark as unknown
                AppStatus::Unknown
            }
        }
    }

    /// Renders the UI
    /// Delegates to view-specific rendering based on current view
    fn render(&self, frame: &mut Frame) {
        // Lock the state for reading during rendering
        // If we can't get the lock, skip this frame
        let Ok(state) = self.state.lock() else {
            return;
        };

        match &state.current_view {
            ViewType::Main => self.render_main_view(frame, &state),
            ViewType::CommandList { .. } => self.render_command_list_view(frame),
            ViewType::LogBrowser { .. } => self.render_log_browser_view(frame),
            ViewType::LogViewer { .. } => self.render_log_viewer_view(frame),
        }

        // Render error message if present
        if let Some(error) = &state.error_message {
            self.render_error_message(frame, error);
        }
    }

    /// Renders the main view
    /// Shows projects, apps, and their status
    fn render_main_view(&self, frame: &mut Frame, state: &AppState) {
        // Delegate to the main view's render method
        self.main_view.render(frame, state, &self.theme);
    }

    /// Renders an error message overlay
    fn render_error_message(&self, frame: &mut Frame, error: &str) {
        let size = frame.size();
        
        // Create a centered popup area
        let popup_area = Self::centered_rect(60, 20, size);
        
        let error_text = Paragraph::new(error)
            .style(Style::default().fg(self.theme.error))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Error")
                    .border_style(Style::default().fg(self.theme.error))
            );
        
        frame.render_widget(error_text, popup_area);
    }

    /// Placeholder for command list view
    fn render_command_list_view(&self, frame: &mut Frame) {
        let size = frame.size();
        let placeholder = Paragraph::new("Command List View - Coming Soon")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Commands"));
        frame.render_widget(placeholder, size);
    }

    /// Placeholder for log browser view
    fn render_log_browser_view(&self, frame: &mut Frame) {
        let size = frame.size();
        let placeholder = Paragraph::new("Log Browser View - Coming Soon")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Logs"));
        frame.render_widget(placeholder, size);
    }

    /// Placeholder for log viewer view
    fn render_log_viewer_view(&self, frame: &mut Frame) {
        let size = frame.size();
        let placeholder = Paragraph::new("Log Viewer View - Coming Soon")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Log Viewer"));
        frame.render_widget(placeholder, size);
    }

    /// Helper function to create a centered rectangle
    /// Used for popups and modals
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}
