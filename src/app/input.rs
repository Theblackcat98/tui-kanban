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

    pub fn insert_str(&mut self, text: &str) {
        self.clamp_cursor();
        self.value.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    /// The start of the word before the cursor, skipping spaces first.
    fn word_start(&self) -> usize {
        let before = &self.value[..self.cursor];
        let trimmed = before.trim_end_matches(char::is_whitespace);
        trimmed
            .char_indices()
            .rev()
            .find(|(_, character)| character.is_whitespace())
            .map_or(0, |(index, character)| index + character.len_utf8())
    }

    /// The end of the word after the cursor, skipping spaces first.
    fn word_end(&self) -> usize {
        let after = &self.value[self.cursor..];
        let skipped = after.len() - after.trim_start_matches(char::is_whitespace).len();
        after[skipped..]
            .char_indices()
            .find(|(_, character)| character.is_whitespace())
            .map_or(self.value.len(), |(index, _)| self.cursor + skipped + index)
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.len();
    }

    /// Applies an editing key, including the readline keys Ctrl+A/E
    /// (start / end), Ctrl+W (delete a word), Ctrl+U / Ctrl+K (delete to
    /// the start / end) and Alt+B/F or Ctrl+←/→ (move by word). Returns
    /// whether the value changed.
    pub fn handle_key(&mut self, key: &KeyEvent) -> bool {
        self.clamp_cursor();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let before = self.value.len();
        match key.code {
            KeyCode::Char('a') if ctrl => self.home(),
            KeyCode::Char('e') if ctrl => self.end(),
            KeyCode::Char('w') if ctrl => {
                let start = self.word_start();
                self.value.replace_range(start..self.cursor, "");
                self.cursor = start;
            }
            KeyCode::Backspace if alt || ctrl => {
                let start = self.word_start();
                self.value.replace_range(start..self.cursor, "");
                self.cursor = start;
            }
            KeyCode::Char('u') if ctrl => {
                self.value.replace_range(..self.cursor, "");
                self.cursor = 0;
            }
            KeyCode::Char('k') if ctrl => self.value.truncate(self.cursor),
            KeyCode::Char('b') if alt => self.cursor = self.word_start(),
            KeyCode::Left if ctrl || alt => self.cursor = self.word_start(),
            KeyCode::Char('f') if alt => self.cursor = self.word_end(),
            KeyCode::Right if ctrl || alt => self.cursor = self.word_end(),
            _ => return self.handle_plain_key(key),
        }
        self.value.len() != before
    }

    fn handle_plain_key(&mut self, key: &KeyEvent) -> bool {
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

    #[test]
    fn readline_keys() {
        let press = |input: &mut TextInput, code: KeyCode, modifiers: KeyModifiers| {
            input.handle_key(&KeyEvent::new(code, modifiers))
        };
        let mut input = TextInput::new("in:done ship  it");
        assert!(press(&mut input, KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(input.value, "in:done ship  ");
        press(&mut input, KeyCode::Char('b'), KeyModifiers::ALT);
        assert_eq!(input.cursor, "in:done ".len());
        press(&mut input, KeyCode::Char('f'), KeyModifiers::ALT);
        assert_eq!(input.cursor, "in:done ship".len());
        press(&mut input, KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(input.cursor, 0);
        press(&mut input, KeyCode::Right, KeyModifiers::CONTROL);
        press(&mut input, KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(input.value, "in:done");
        press(&mut input, KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(input.value, "");
        input.insert_str("héllo");
        assert_eq!(input.cursor, input.value.len());
    }
}
