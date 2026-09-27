use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A single-line text field: a string and a cursor at a byte offset that
/// is always on a character boundary.
#[derive(Clone, Debug)]
pub struct TextInput {
    pub value: String,
    pub cursor: usize,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        let mut input = Self {
            value: value.into(),
            cursor: 0,
        };
        input.cursor = input.value.len();
        input
    }

    pub fn insert(&mut self, character: char) {
        self.clamp_cursor();
        self.value.insert(self.cursor, character);
        self.cursor += character.len_utf8();
    }

    pub fn backspace(&mut self) {
        self.clamp_cursor();
        if self.cursor == 0 {
            return;
        }
        let previous = self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
        self.value.remove(previous);
        self.cursor = previous;
    }

    pub fn delete(&mut self) {
        self.clamp_cursor();
        if self.cursor < self.value.len() {
            self.value.remove(self.cursor);
        }
    }

    pub fn move_left(&mut self) {
        self.clamp_cursor();
        self.cursor = self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
    }

    pub fn move_right(&mut self) {
        self.clamp_cursor();
        if self.cursor < self.value.len() {
            self.cursor += self.value[self.cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(0);
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.len();
    }

    /// Applies an editing key. Returns whether the value changed.
    pub fn handle_key(&mut self, key: &KeyEvent) -> bool {
        match key.code {
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Home => self.home(),
            KeyCode::End => self.end(),
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.insert(character)
            }
            _ => return false,
        }
        matches!(
            key.code,
            KeyCode::Backspace | KeyCode::Delete | KeyCode::Char(_)
        )
    }

    fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.value.len());
        while self.cursor > 0 && !self.value.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_handles_utf8_boundaries() {
        let mut input = TextInput::new("é");
        input.backspace();
        assert_eq!(input.value, "");
        input.insert('🦀');
        input.move_left();
        input.insert('x');
        assert_eq!(input.value, "x🦀");
    }
}
