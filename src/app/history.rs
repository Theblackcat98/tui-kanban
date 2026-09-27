//! Undo and redo. Every board change already copies the board before
//! changing it, so undoing is swapping that copy back in.

use uuid::Uuid;

use crate::domain::Board;

/// How many changes can be undone. Boards are small, so full copies are
/// cheap; if they grow large, store the inverse of each change instead.
pub const LIMIT: usize = 100;

#[derive(Clone, Debug)]
pub struct Entry {
    pub board: Board,
    /// What the change did, as in "move 'Ship it' → Done".
    pub label: String,
    /// The selection to restore with this board.
    pub selected: Option<Uuid>,
}

#[derive(Clone, Debug, Default)]
pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

impl History {
    /// Records the board as it was before a change. A new change can't be
    /// redone past, so this clears the redo stack.
    pub fn record(&mut self, entry: Entry) {
        if self.undo.len() == LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(entry);
        self.redo.clear();
    }

    /// Takes the last change to undo. `current` is the board as it is now,
    /// kept so the undo can itself be redone.
    pub fn undo(&mut self, current: Entry) -> Option<Entry> {
        let entry = self.undo.pop()?;
        self.redo.push(Entry {
            label: entry.label.clone(),
            ..current
        });
        Some(entry)
    }

    pub fn redo(&mut self, current: Entry) -> Option<Entry> {
        let entry = self.redo.pop()?;
        self.undo.push(Entry {
            label: entry.label.clone(),
            ..current
        });
        Some(entry)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> Entry {
        Entry {
            board: Board {
                name: name.to_owned(),
                ..Board::default()
            },
            label: format!("change to {name}"),
            selected: None,
        }
    }

    #[test]
    fn undo_and_redo_swap_boards() {
        let mut history = History::default();
        history.record(entry("a"));
        let undone = history.undo(entry("b")).unwrap();
        assert_eq!(undone.board.name, "a");
        assert_eq!(undone.label, "change to a");
        let redone = history.redo(entry("a")).unwrap();
        assert_eq!(redone.board.name, "b");
        // The redo entry carries the label of the change it redoes.
        assert_eq!(redone.label, "change to a");
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn a_new_change_clears_redo() {
        let mut history = History::default();
        history.record(entry("a"));
        history.undo(entry("b"));
        assert!(history.can_redo());
        history.record(entry("c"));
        assert!(!history.can_redo());
    }

    #[test]
    fn history_is_capped() {
        let mut history = History::default();
        for index in 0..LIMIT + 10 {
            history.record(entry(&index.to_string()));
        }
        let mut count = 0;
        let mut oldest = String::new();
        while let Some(entry) = history.undo(entry("now")) {
            count += 1;
            oldest = entry.board.name;
        }
        assert_eq!(count, LIMIT);
        assert_eq!(oldest, "10");
    }
}
