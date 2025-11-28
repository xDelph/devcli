use crate::tui::theme::Theme;
use crate::tui::views::main_view::MainView;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

impl MainView {
    /// Renders the delete confirmation popup
    pub(crate) fn render_delete_confirmation_popup(&self, frame: &mut Frame, theme: &Theme) {
        let area = frame.area();

        // Parse message lines
        let message_lines: Vec<&str> = self.delete_confirm_message.lines().collect();
        let max_line_len = message_lines.iter().map(|l| l.len()).max().unwrap_or(40);

        // Calculate popup size with generous padding to avoid text cutoff
        // Use a larger multiplier and minimum to ensure text fits
        let popup_width = ((max_line_len as f32 * 1.2) as u16 + 16).max(70);
        let popup_height = (message_lines.len() + 6) as u16;

        // Center the popup
        let popup_area = Rect {
            x: (area.width.saturating_sub(popup_width)) / 2,
            y: (area.height.saturating_sub(popup_height)) / 2,
            width: popup_width.min(area.width),
            height: popup_height.min(area.height),
        };

        // Build popup content with proper padding
        let mut lines = Vec::new();
        lines.push(Line::from(""));

        // Add message lines with left padding
        for line in message_lines {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    line.to_string(),
                    theme.style_text().add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Press ", theme.style_text_dim()),
            Span::styled(
                "Y",
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to confirm, ", theme.style_text_dim()),
            Span::styled(
                "N",
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" or ", theme.style_text_dim()),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to cancel", theme.style_text_dim()),
        ]));

        let popup = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.style_text_primary())
                .title(" Confirm Delete ")
                .style(theme.style_bg_default()),
        );

        frame.render_widget(popup, popup_area);
    }
}
