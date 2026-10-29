use ratatui::{
    style::{Color, Style},
    widgets::Paragraph,
};

/// A reusable text editor component
#[derive(Debug, Clone, Default)]
pub struct TextEditor {
    /// The text content
    content: String,
    /// Current cursor position, in characters (not bytes).
    ///
    /// It used to be a byte offset moved by ±1 and handed to `String::insert`
    /// / `remove`, which panic off a char boundary: typing `é` then any other
    /// key, or Backspace in a path such as `/Users/x/Développement`, crashed
    /// the TUI. The renderers already read it as a character count.
    cursor_position: usize,
}

impl TextEditor {
    /// Creates a new empty text editor
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new text editor with initial content
    pub fn with_content(content: String) -> Self {
        let cursor_position = content.chars().count();
        Self {
            content,
            cursor_position,
        }
    }

    fn char_len(&self) -> usize {
        self.content.chars().count()
    }

    /// Byte offset of the character at `char_index` (or the end).
    fn byte_index(&self, char_index: usize) -> usize {
        self.content
            .char_indices()
            .nth(char_index)
            .map(|(byte, _)| byte)
            .unwrap_or(self.content.len())
    }

    /// Inserts a character at the current cursor position
    pub fn insert_char(&mut self, c: char) {
        self.cursor_position = self.cursor_position.min(self.char_len());
        let at = self.byte_index(self.cursor_position);
        self.content.insert(at, c);
        self.cursor_position += 1;
    }

    /// Deletes the character before the cursor (backspace)
    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 && self.cursor_position <= self.char_len() {
            self.cursor_position -= 1;
            let at = self.byte_index(self.cursor_position);
            self.content.remove(at);
        }
    }

    /// Moves the cursor left
    pub fn move_cursor_left(&mut self) {
        self.cursor_position = self.cursor_position.saturating_sub(1);
    }

    /// Moves the cursor right
    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.char_len() {
            self.cursor_position += 1;
        }
    }

    /// Sets the cursor to the end of the text
    pub fn move_cursor_to_end(&mut self) {
        self.cursor_position = self.char_len();
    }

    /// Sets the cursor to the beginning of the text
    pub fn move_cursor_to_start(&mut self) {
        self.cursor_position = 0;
    }

    /// Returns the current content
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the current cursor position
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Sets the content and moves cursor to end
    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.cursor_position = self.char_len();
    }

    /// Clears the content and resets cursor
    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_position = 0;
    }

    /// Renders the text editor as a Paragraph
    /// Note: This returns a Paragraph widget, it doesn't render directly to a frame
    /// The caller should render it using frame.render_widget
    pub fn as_widget(&self, is_focused: bool) -> Paragraph<'static> {
        let style = if is_focused {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        Paragraph::new(self.content.clone()).style(style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_char() {
        let mut editor = TextEditor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        assert_eq!(editor.content(), "ab");
        assert_eq!(editor.cursor_position(), 2);
    }

    #[test]
    fn test_delete_char() {
        let mut editor = TextEditor::with_content("abc".to_string());
        editor.delete_char();
        assert_eq!(editor.content(), "ab");
        assert_eq!(editor.cursor_position(), 2);

        editor.move_cursor_left();
        editor.delete_char();
        assert_eq!(editor.content(), "b");
        assert_eq!(editor.cursor_position(), 0);
    }

    #[test]
    fn test_cursor_movement() {
        let mut editor = TextEditor::with_content("abc".to_string());

        editor.move_cursor_left();
        assert_eq!(editor.cursor_position(), 2);

        editor.move_cursor_left();
        editor.move_cursor_left();
        assert_eq!(editor.cursor_position(), 0);

        editor.move_cursor_left(); // Should not go below 0
        assert_eq!(editor.cursor_position(), 0);

        editor.move_cursor_right();
        assert_eq!(editor.cursor_position(), 1);

        editor.move_cursor_to_end();
        assert_eq!(editor.cursor_position(), 3);

        editor.move_cursor_right(); // Should not go past end
        assert_eq!(editor.cursor_position(), 3);
    }

    #[test]
    fn test_non_ascii_editing_does_not_panic() {
        let mut editor = TextEditor::new();
        editor.insert_char('é');
        editor.insert_char('x');
        editor.insert_char('🚀');
        assert_eq!(editor.content(), "éx🚀");
        assert_eq!(editor.cursor_position(), 3);

        editor.move_cursor_left();
        editor.delete_char();
        assert_eq!(editor.content(), "é🚀");

        let mut path = TextEditor::with_content("/Users/x/Développement".to_string());
        assert_eq!(path.cursor_position(), 22);
        // Step back to just after the `é` and replace it.
        for _ in 0..11 {
            path.move_cursor_left();
        }
        path.delete_char();
        path.insert_char('E');
        assert_eq!(path.content(), "/Users/x/DEveloppement");
    }
}
