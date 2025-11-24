use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

pub type TuiTerminal = Terminal<CrosstermBackend<io::Stdout>>;

/// Sets up the terminal for TUI mode
/// Enables raw mode and enters alternate screen
pub fn setup_terminal() -> Result<TuiTerminal> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    // Don't enable mouse capture to avoid mouse event codes appearing
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("Failed to create terminal")
}

/// Restores the terminal to normal mode
/// Disables raw mode and leaves alternate screen
pub fn restore_terminal(terminal: &mut TuiTerminal) -> Result<()> {
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to show cursor")?;
    Ok(())
}
