//! A worker thread that writes the board, so a slow disk never holds up
//! a key press.
//!
//! The runtime decides *when* to save (see `app::persist`); the worker
//! only writes, one job at a time, in the order the jobs were sent.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::{Fingerprint, JsonStore, StoreError};
use crate::domain::Board;

pub struct Job {
    pub board: Board,
    /// The file as it was last read or written here. Unless `force` is
    /// set, the job fails with [`StoreError::Conflict`] if the file has
    /// changed since.
    pub expected: Option<Fingerprint>,
    pub force: bool,
    /// Passed back with the outcome, to tell which change was saved.
    pub revision: u64,
}

pub struct Outcome {
    pub revision: u64,
    pub result: Result<Fingerprint, StoreError>,
}

pub struct Saver {
    jobs: Option<Sender<Job>>,
    outcomes: Receiver<Outcome>,
    worker: Option<JoinHandle<()>>,
}

impl Saver {
    pub fn spawn(store: JsonStore) -> Self {
        let (jobs, job_queue) = mpsc::channel::<Job>();
        let (done, outcomes) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("tui-kanban saver".to_owned())
            .spawn(move || {
                for job in job_queue {
                    let result = if job.force {
                        store.save(&job.board)
                    } else {
                        store.save_unless_changed(&job.board, job.expected)
                    };
                    let outcome = Outcome {
                        revision: job.revision,
                        result,
                    };
                    if done.send(outcome).is_err() {
                        break;
                    }
                }
            })
            .ok();
        Self {
            jobs: worker.is_some().then_some(jobs),
            outcomes,
            worker,
        }
    }

    /// Hands a job to the worker. If the worker has stopped, the job comes
    /// straight back as a failure.
    pub fn submit(&self, job: Job) -> Result<(), Outcome> {
        let revision = job.revision;
        let stopped = || Outcome {
            revision,
            result: Err(StoreError::Write("the background saver stopped".to_owned())),
        };
        match &self.jobs {
            Some(jobs) => jobs.send(job).map_err(|_| stopped()),
            None => Err(stopped()),
        }
    }

    /// A finished job's outcome, if one is ready.
    pub fn try_outcome(&self) -> Option<Outcome> {
        self.outcomes.try_recv().ok()
    }

    /// Waits up to `timeout` for a job to finish.
    pub fn wait(&self, timeout: Duration) -> Option<Outcome> {
        self.outcomes.recv_timeout(timeout).ok()
    }
}

impl Drop for Saver {
    /// Lets the worker finish the job it is on, so a save is never cut
    /// off halfway (the write is atomic, but the change would be lost).
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAIT: Duration = Duration::from_secs(5);

    #[test]
    fn jobs_are_saved_in_order_and_report_back() {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("board.json"));
        let saver = Saver::spawn(store.clone());
        let mut board = Board::default();
        let job = |board: &Board, revision, expected| Job {
            board: board.clone(),
            expected,
            force: false,
            revision,
        };
        saver.submit(job(&board, 1, None)).ok().unwrap();
        let first = saver.wait(WAIT).unwrap();
        assert_eq!(first.revision, 1);
        let fingerprint = first.result.unwrap();
        board.add_task(0, "Saved later", "", 0).unwrap();
        saver
            .submit(job(&board, 2, Some(fingerprint)))
            .ok()
            .unwrap();
        let second = saver.wait(WAIT).unwrap();
        assert_eq!(second.revision, 2);
        assert!(second.result.is_ok());
        assert_eq!(store.load().unwrap().unwrap(), board);
        // The file no longer matches the first fingerprint.
        saver
            .submit(job(&board, 3, Some(fingerprint)))
            .ok()
            .unwrap();
        let conflict = saver.wait(WAIT).unwrap();
        assert!(matches!(conflict.result, Err(StoreError::Conflict)));
        // Unless the job forces it.
        saver
            .submit(Job {
                force: true,
                ..job(&board, 4, Some(fingerprint))
            })
            .ok()
            .unwrap();
        assert!(saver.wait(WAIT).unwrap().result.is_ok());
        assert!(saver.try_outcome().is_none());
    }

    #[test]
    fn dropping_the_saver_finishes_the_current_job() {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("board.json"));
        let saver = Saver::spawn(store.clone());
        saver
            .submit(Job {
                board: Board::default(),
                expected: None,
                force: false,
                revision: 1,
            })
            .ok()
            .unwrap();
        drop(saver);
        assert_eq!(store.load().unwrap().unwrap(), Board::default());
    }
}
