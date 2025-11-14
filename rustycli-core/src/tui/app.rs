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
use std::time::Duration;

/// The main TUI application struct
/// Manages the terminal, state, and event loop
pub struct TuiApp {
    /// Current application state
    state: AppState,
    /// Visual theme for the UI
    theme: Theme,
    /// Main view instance
    main_view: MainView,
    /// Process tracker for checking app status
    /// Will be used in future tasks for status updates
    #[allow(dead_code)]
    process_tracker: ProcessTracker,
    /// Flag to indicate the app should quit
    should_quit: bool,
}

impl TuiApp {
    /// Creates a new TUI application
    /// Loads configuration and initializes state
    pub fn new() -> Result<Self> {
        // Load configuration from disk
        let config = load_config().context("Failed to load configuration")?;
        
        // Initialize process tracker
        let process_tracker = ProcessTracker::new()?;
        
        // Create initial state from config
        let state = AppState::from_config(&config, &process_tracker)
            .context("Failed to create application state")?;
        
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
    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode().context("Failed to enable raw mode")?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to enter alternate screen")?;
        
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

        // Run the main loop
        let result = self.run_event_loop(&mut terminal);

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

        // View-specific handling
        match &self.state.current_view {
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
        self.state.current_view = ViewType::Main;
    }

    /// Renders the UI
    /// Delegates to view-specific rendering based on current view
    fn render(&self, frame: &mut Frame) {
        match &self.state.current_view {
            ViewType::Main => self.render_main_view(frame),
            ViewType::CommandList { .. } => self.render_command_list_view(frame),
            ViewType::LogBrowser { .. } => self.render_log_browser_view(frame),
            ViewType::LogViewer { .. } => self.render_log_viewer_view(frame),
        }

        // Render error message if present
        if let Some(error) = &self.state.error_message {
            self.render_error_message(frame, error);
        }
    }

    /// Renders the main view
    /// Shows projects, apps, and their status
    fn render_main_view(&self, frame: &mut Frame) {
        // Delegate to the main view's render method
        self.main_view.render(frame, &self.state, &self.theme);
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
