use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Defines keyboard shortcuts for the application
pub struct KeyBindings;

impl KeyBindings {
    /// Check if the key event triggers a quit action
    pub fn is_quit(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Char('q'))
            || (matches!(key.code, KeyCode::Char('c'))
                && key.modifiers.contains(KeyModifiers::CONTROL))
    }

    /// Check if the key event triggers the help overlay
    pub fn is_help(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Char('?'))
    }

    /// Check if the key event triggers a back action (Esc)
    pub fn is_back(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Esc)
    }

    /// Check if the key event triggers tab switching to Status (1)
    pub fn is_tab_status(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Char('1') | KeyCode::Char('&'))
    }

    /// Check if the key event triggers tab switching to Commands (2)
    pub fn is_tab_commands(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Char('2') | KeyCode::Char('é'))
    }

    /// Check if the key event triggers tab switching to Logs (3)
    pub fn is_tab_logs(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Char('3') | KeyCode::Char('"'))
    }

    /// Check if the key event triggers tab switching to Config (4)
    pub fn is_tab_config(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Char('4') | KeyCode::Char('\''))
    }

    /// Check if the key event triggers cycling tabs
    pub fn is_tab_cycle(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Tab)
    }

    /// Check if the key event triggers panel focus toggle
    pub fn is_panel_toggle(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Left | KeyCode::Right)
    }

    /// Check if the key event triggers fast navigation up (Shift+Up)
    pub fn is_fast_up(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Up) && key.modifiers.contains(KeyModifiers::SHIFT)
    }

    /// Check if the key event triggers fast navigation down (Shift+Down)
    pub fn is_fast_down(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Down) && key.modifiers.contains(KeyModifiers::SHIFT)
    }

    /// Check if the key event triggers navigation up
    pub fn is_up(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Up | KeyCode::Char('k'))
    }

    /// Check if the key event triggers navigation down
    pub fn is_down(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Down | KeyCode::Char('j'))
    }

    /// Check if the key event triggers expand/collapse
    pub fn is_expand(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Char(' '))
    }

    /// Check if the key event triggers enter/confirm
    pub fn is_enter(key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Enter)
    }
}
