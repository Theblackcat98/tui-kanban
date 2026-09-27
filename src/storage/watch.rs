//! Noticing when the board file changes on disk, because another program
//! (an editor, `git pull`, a second tui-kanban) wrote it.
//!
//! The watcher only says *that* something happened; the runtime reads the
//! file and compares fingerprints to tell other programs' changes from its
//! own saves.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::ffi::OsString;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};

pub struct Watcher {
    // Kept alive for as long as the board is watched.
    _watcher: RecommendedWatcher,
    events: Receiver<()>,
}

impl Watcher {
    /// Watches the directory holding `path` (saves replace the file, so
    /// watching the file itself would stop at the first save) for changes
    /// to that one file.
    pub fn new(path: &Path) -> notify::Result<Self> {
        let directory = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let directory = directory
            .canonicalize()
            .unwrap_or_else(|_| directory.to_owned());
        let name: OsString = path.file_name().unwrap_or_default().to_owned();
        let (sender, events) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
            // After an error events may have been missed, so look at the
            // file to be safe.
            let relevant = event.map_or(true, |event| changes(&event, &name));
            if relevant {
                let _ = sender.send(());
            }
        })?;
        watcher.watch(&directory, RecursiveMode::NonRecursive)?;
        Ok(Self {
            _watcher: watcher,
            events,
        })
    }

    /// Whether the file may have changed since this was last called.
    pub fn changed(&self) -> bool {
        let mut changed = false;
        while self.events.try_recv().is_ok() {
            changed = true;
        }
        changed
    }
}

/// Whether `event` may have changed the file called `name`. Reading the
/// file, as the runtime does to check it, doesn't.
fn changes(event: &Event, name: &OsString) -> bool {
    !matches!(event.kind, EventKind::Access(_))
        && event
            .paths
            .iter()
            .any(|path| path.file_name() == Some(name.as_os_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, Instant};

    fn eventually(watcher: &Watcher) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if watcher.changed() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn notices_changes_to_the_file_only() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("board.json");
        fs::write(&path, "{}").unwrap();
        let watcher = Watcher::new(&path).unwrap();
        fs::write(&path, "{\"changed\": true}").unwrap();
        assert!(eventually(&watcher));
        // A save replaces the file; the watcher keeps working after it.
        crate::storage::JsonStore::new(&path)
            .save(&crate::domain::Board::default())
            .unwrap();
        assert!(eventually(&watcher));
    }

    #[test]
    fn only_changes_to_the_board_file_count() {
        use notify::event::{AccessKind, CreateKind, ModifyKind};
        let name = OsString::from("board.json");
        let event = |kind, path: &str| Event::new(kind).add_path(path.into());
        let modify = EventKind::Modify(ModifyKind::Any);
        assert!(changes(&event(modify, "/b/board.json"), &name));
        assert!(changes(
            &event(EventKind::Create(CreateKind::File), "/b/board.json"),
            &name
        ));
        assert!(!changes(&event(modify, "/b/other.json"), &name));
        assert!(!changes(
            &event(EventKind::Access(AccessKind::Any), "/b/board.json"),
            &name
        ));
    }
}
