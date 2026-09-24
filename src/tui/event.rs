use ratatui::crossterm::event::KeyEvent;

pub fn accepts_key(key: &KeyEvent) -> bool {
    matches!(
        key.kind,
        ratatui::crossterm::event::KeyEventKind::Press
            | ratatui::crossterm::event::KeyEventKind::Repeat
    )
}
