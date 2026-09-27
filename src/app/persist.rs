//! The runtime side of saving. `update` only says that the board changed
//! ([`Effect::Save`](super::Effect::Save)); this decides when to write it,
//! hands the writing to a background [`Saver`], and notices when another
//! program changes the file.
//!
//! - A change is written [`SAVE_DELAY`] after it is made, together with
//!   any changes made in the meantime.
//! - A failed save keeps the change and is retried, waiting longer each
//!   time, up to [`RETRY_MAX`].
//! - Every save first checks that the file is still what was last read or
//!   written here. If another program changed it, nothing is written and
//!   the user decides which version to keep.
//! - When the file changes on disk and nothing here is unsaved, the new
//!   version is loaded.

use std::time::{Duration, Instant};

use super::action::Action;
use super::model::{Model, SaveState};
use crate::domain::Board;
use crate::storage::{Fingerprint, Job, JsonStore, Outcome, Saver, StoreError, Watcher};

/// How long a change waits before it is written, so a burst of changes
/// (holding a key down) is one save.
pub const SAVE_DELAY: Duration = Duration::from_millis(150);
/// How long to wait before retrying the first failed save. Each failure
/// doubles it.
pub const RETRY_FIRST: Duration = Duration::from_secs(1);
pub const RETRY_MAX: Duration = Duration::from_secs(30);
/// How often the event loop checks on a save in progress.
const POLL_WHILE_SAVING: Duration = Duration::from_millis(10);
/// How long to let the file settle after a change notice before reading
/// it, since other programs may write it in several steps.
pub const SETTLE: Duration = Duration::from_millis(100);
/// How long quitting waits for a save to finish.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Persistence {
    store: JsonStore,
    saver: Saver,
    watcher: Option<Watcher>,
    /// The file as it was last read or written here, or `None` if there
    /// was no file.
    known: Option<Fingerprint>,
    /// When to write the changes waiting to be saved.
    due: Option<Instant>,
    /// Whether the next save overwrites the file even if it changed.
    force: bool,
    /// Whether the saver is writing.
    in_flight: bool,
    retry_delay: Duration,
    /// When to look at the file after a change notice.
    check_due: Option<Instant>,
    /// A changed file that couldn't be loaded, so it is reported once.
    unreadable: Option<Fingerprint>,
}

impl Persistence {
    /// Loads the board (or `new_board`, if there is no file) and starts
    /// the saver.
    pub fn open(store: JsonStore, new_board: Board) -> Result<(Self, Board), StoreError> {
        let loaded = store.read()?;
        let known = loaded.as_ref().map(|loaded| loaded.fingerprint);
        let board = loaded.map_or(new_board, |loaded| loaded.board);
        let persistence = Self {
            saver: Saver::spawn(store.clone()),
            store,
            watcher: None,
            known,
            due: None,
            force: false,
            in_flight: false,
            retry_delay: RETRY_FIRST,
            check_due: None,
            unreadable: None,
        };
        Ok((persistence, board))
    }

    pub fn store(&self) -> &JsonStore {
        &self.store
    }

    /// Starts watching the file for changes by other programs. Without a
    /// watcher those changes still aren't overwritten, but they are only
    /// noticed at the next save.
    pub fn watch(&mut self) {
        self.watcher = Watcher::new(self.store.path()).ok();
    }

    /// The board changed: save it soon.
    pub fn request_save(&mut self, now: Instant) {
        self.due.get_or_insert(now + SAVE_DELAY);
    }

    /// Save now, even though the file changed on disk.
    pub fn overwrite(&mut self, now: Instant) {
        self.force = true;
        self.due = Some(now);
    }

    /// The file may have changed: look at it once it has settled.
    pub fn notice_change(&mut self, now: Instant) {
        self.check_due = Some(now + SETTLE);
    }

    /// Outcomes of finished saves, as actions.
    pub fn finished(&mut self, now: Instant) -> Vec<Action> {
        let mut actions = Vec::new();
        while let Some(outcome) = self.saver.try_outcome() {
            actions.push(self.finish(outcome, now));
        }
        if self.watcher.as_ref().is_some_and(Watcher::changed) {
            self.notice_change(now);
        }
        actions
    }

    fn finish(&mut self, outcome: Outcome, now: Instant) -> Action {
        self.in_flight = false;
        match outcome.result {
            Ok(fingerprint) => {
                self.known = Some(fingerprint);
                self.force = false;
                self.retry_delay = RETRY_FIRST;
                Action::SaveFinished {
                    revision: outcome.revision,
                    result: Ok(()),
                }
            }
            Err(StoreError::Conflict) => {
                self.due = None;
                Action::Conflict
            }
            Err(error) => {
                self.due = Some(now + self.retry_delay);
                self.retry_delay = (self.retry_delay * 2).min(RETRY_MAX);
                Action::SaveFinished {
                    revision: outcome.revision,
                    result: Err(error.to_string()),
                }
            }
        }
    }

    /// If the file changed on disk, loads it, or reports a conflict when
    /// there are unsaved changes here. Waits while a save is being written,
    /// which changes the file too.
    pub fn check_disk(&mut self, model: &Model, now: Instant) -> Option<Action> {
        if self.in_flight || self.check_due.is_none_or(|due| due > now) {
            return None;
        }
        self.check_due = None;
        // A file that can't be read or was deleted is written again by the
        // next save, which reports any error.
        let current = self.store.fingerprint().ok()??;
        if Some(current) == self.known || Some(current) == self.unreadable {
            return None;
        }
        if model.session.save_state != SaveState::Saved {
            self.due = None;
            return Some(Action::Conflict);
        }
        match self.store.read() {
            Ok(Some(loaded)) => {
                self.known = Some(loaded.fingerprint);
                self.unreadable = None;
                Some(Action::Reloaded(loaded.board))
            }
            Ok(None) => None,
            Err(error) => {
                self.unreadable = Some(current);
                Some(Action::ReloadFailed(error.to_string()))
            }
        }
    }

    /// Hands the board to the saver if a save is due and none is being
    /// written. Returns an action only if the saver has stopped.
    pub fn submit_due(&mut self, model: &Model, now: Instant) -> Option<Action> {
        if self.in_flight || self.due.is_none_or(|due| due > now) {
            return None;
        }
        self.submit(model, now)
    }

    fn submit(&mut self, model: &Model, now: Instant) -> Option<Action> {
        self.due = None;
        let job = Job {
            board: model.board.clone(),
            expected: self.known,
            force: self.force,
            revision: model.session.revision,
        };
        match self.saver.submit(job) {
            Ok(()) => {
                self.in_flight = true;
                None
            }
            Err(outcome) => Some(self.finish(outcome, now)),
        }
    }

    /// Loads the board from disk, dropping any unsaved changes.
    pub fn reload(&mut self) -> Action {
        if self.in_flight {
            // Its outcome doesn't matter: the board is being replaced.
            self.in_flight = false;
            if let Some(Outcome {
                result: Ok(fingerprint),
                ..
            }) = self.saver.wait(FLUSH_TIMEOUT)
            {
                self.known = Some(fingerprint);
            }
        }
        self.due = None;
        self.force = false;
        match self.store.read() {
            Ok(Some(loaded)) => {
                self.known = Some(loaded.fingerprint);
                self.unreadable = None;
                Action::Reloaded(loaded.board)
            }
            Ok(None) => Action::ReloadFailed("the board file no longer exists".to_owned()),
            Err(error) => Action::ReloadFailed(error.to_string()),
        }
    }

    /// Saves everything now, waiting for the saver: the save being written
    /// and then any changes still waiting (retrying a failed save once
    /// more). Used before quitting.
    pub fn flush(&mut self, model: &Model, now: Instant) -> Vec<Action> {
        let mut actions = Vec::new();
        if self.in_flight {
            actions.extend(self.wait(now));
        }
        if self.due.is_some() {
            actions.extend(self.submit(model, now));
            if self.in_flight {
                actions.extend(self.wait(now));
            }
        }
        actions
    }

    fn wait(&mut self, now: Instant) -> Option<Action> {
        let outcome = self.saver.wait(FLUSH_TIMEOUT)?;
        Some(self.finish(outcome, now))
    }

    /// How long the event loop may sleep before this needs to run again.
    pub fn next_timeout(&self, now: Instant) -> Option<Duration> {
        if self.in_flight {
            return Some(POLL_WHILE_SAVING);
        }
        [self.due, self.check_due]
            .into_iter()
            .flatten()
            .map(|due| due.saturating_duration_since(now))
            .min()
    }
}
