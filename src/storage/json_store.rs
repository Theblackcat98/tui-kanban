use crate::domain::{Board, BoardError};
use atomic_write_file::AtomicWriteFile;
use std::fmt;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Read(PathBuf, std::io::Error),
    Json(serde_json::Error),
    Board(BoardError),
    Write(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "board I/O error: {error}"),
            Self::Read(path, error) => {
                write!(formatter, "could not read {}: {error}", path.display())
            }
            Self::Json(error) => write!(formatter, "invalid board JSON: {error}"),
            Self::Board(error) => write!(formatter, "invalid board data: {error}"),
            Self::Write(message) => write!(formatter, "could not write board: {message}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<BoardError> for StoreError {
    fn from(error: BoardError) -> Self {
        Self::Board(error)
    }
}

#[derive(Clone, Debug)]
pub struct JsonStore {
    path: PathBuf,
}

impl JsonStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the board, or returns `Ok(None)` if the file does not exist.
    /// Any other problem, such as permission denied, is an error: treating
    /// it as missing would open an empty board that could overwrite the
    /// real one.
    pub fn load(&self) -> Result<Option<Board>, StoreError> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(StoreError::Read(self.path.clone(), error)),
        };
        let board = serde_json::from_str::<Board>(&contents)?;
        board.validate()?;
        Ok(Some(board))
    }

    pub fn load_or_default(&self) -> Result<Board, StoreError> {
        Ok(self.load()?.unwrap_or_default())
    }

    pub fn save(&self, board: &Board) -> Result<(), StoreError> {
        board.validate()?;
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }

        let mut file = AtomicWriteFile::open(&self.path)
            .map_err(|error| StoreError::Write(error.to_string()))?;
        serde_json::to_writer_pretty(&mut file, board)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        file.commit()
            .map_err(|error| StoreError::Write(error.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Board;
    use std::fs;

    #[test]
    fn saves_and_loads_a_board() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("board.json");
        let store = JsonStore::new(&path);
        let mut board = Board::default();
        board.add_task(0, "Persist me", "Round trip", 0).unwrap();
        store.save(&board).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded, board);
        assert!(fs::read_to_string(path).unwrap().lines().count() > 5);
    }

    #[test]
    fn missing_file_loads_as_none() {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("missing.json"));
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn unreadable_file_is_an_error_not_missing() {
        let directory = tempfile::tempdir().unwrap();
        // A directory where the file should be can't be read as a file,
        // on every platform and even when running as root.
        let path = directory.path().join("board.json");
        fs::create_dir(&path).unwrap();
        let error = JsonStore::new(&path).load().unwrap_err();
        assert!(matches!(error, StoreError::Read(..)), "{error:?}");
        assert!(error.to_string().contains("board.json"), "{error}");
    }

    #[test]
    fn demo_board_is_valid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/demo-board.json");
        let board = JsonStore::new(path).load().unwrap().unwrap();
        assert_eq!(board.name, "Demo Board");
        assert_eq!(board.task_count(), 7);
    }

    #[test]
    fn malformed_file_is_not_overwritten_on_load() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("board.json");
        fs::write(&path, "{not valid json").unwrap();
        let store = JsonStore::new(&path);
        assert!(store.load().is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "{not valid json");
    }
}
