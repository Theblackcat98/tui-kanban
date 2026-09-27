use super::migrate::{self, MIGRATIONS, Migration};
use crate::domain::{Board, BoardError, SCHEMA_VERSION};
use atomic_write_file::AtomicWriteFile;
use std::fmt;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Read(PathBuf, std::io::Error),
    Json(serde_json::Error),
    Board(BoardError),
    Write(String),
    /// The file changed on disk since it was last read or written here,
    /// so saving would overwrite someone else's changes.
    Conflict,
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
            Self::Conflict => formatter.write_str("the board file changed on disk"),
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

/// What the board file held when it was last read or written here, used to
/// tell whether anyone else has changed it since.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fingerprint(u64);

impl Fingerprint {
    pub fn of(contents: &[u8]) -> Self {
        let mut hasher = DefaultHasher::new();
        hasher.write(contents);
        Self(hasher.finish())
    }
}

/// A board read from disk, with the fingerprint of the file it came from.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub board: Board,
    pub fingerprint: Fingerprint,
}

#[derive(Clone, Debug)]
pub struct JsonStore {
    path: PathBuf,
    migrations: &'static [Migration],
}

impl JsonStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            migrations: MIGRATIONS,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Where a file at schema `version` is backed up before it is upgraded.
    pub fn backup_path(&self, version: u32) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".bak-v{version}"));
        self.path.with_file_name(name)
    }

    /// Loads the board, or returns `Ok(None)` if the file does not exist.
    /// Any other problem, such as permission denied, is an error: treating
    /// it as missing would open an empty board that could overwrite the
    /// real one.
    pub fn load(&self) -> Result<Option<Board>, StoreError> {
        Ok(self.read()?.map(|loaded| loaded.board))
    }

    pub fn load_or_default(&self) -> Result<Board, StoreError> {
        Ok(self.load()?.unwrap_or_default())
    }

    /// Loads the board with its file's fingerprint, or `Ok(None)` if the
    /// file does not exist. A file from an older version is backed up and
    /// upgraded in memory.
    pub fn read(&self) -> Result<Option<Loaded>, StoreError> {
        let Some(contents) = self.contents()? else {
            return Ok(None);
        };
        let board = self.parse(&contents)?;
        Ok(Some(Loaded {
            board,
            fingerprint: Fingerprint::of(&contents),
        }))
    }

    /// The fingerprint of the file as it is now, or `None` if it doesn't
    /// exist.
    pub fn fingerprint(&self) -> Result<Option<Fingerprint>, StoreError> {
        Ok(self.contents()?.map(|contents| Fingerprint::of(&contents)))
    }

    fn contents(&self) -> Result<Option<Vec<u8>>, StoreError> {
        match fs::read(&self.path) {
            Ok(contents) => Ok(Some(contents)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StoreError::Read(self.path.clone(), error)),
        }
    }

    fn parse(&self, contents: &[u8]) -> Result<Board, StoreError> {
        let mut value: serde_json::Value = serde_json::from_slice(contents)?;
        let version = migrate::version_of(&value)?;
        if version != SCHEMA_VERSION {
            if version < SCHEMA_VERSION {
                self.back_up(contents, version)?;
            }
            value = migrate::upgrade(value, version, SCHEMA_VERSION, self.migrations)?;
        }
        let board = serde_json::from_value::<Board>(value)?;
        board.validate()?;
        Ok(board)
    }

    /// Keeps a copy of a file before it is upgraded. An existing backup is
    /// left alone: it holds the file as it was before the first upgrade.
    fn back_up(&self, contents: &[u8], version: u32) -> Result<(), StoreError> {
        let backup = self.backup_path(version);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
        {
            Ok(mut file) => {
                file.write_all(contents)?;
                file.sync_all()?;
                Ok(())
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(StoreError::Write(format!(
                "could not back up the board to {}: {error}",
                backup.display()
            ))),
        }
    }

    /// Writes the board, replacing whatever is on disk, and returns the
    /// new file's fingerprint.
    pub fn save(&self, board: &Board) -> Result<Fingerprint, StoreError> {
        board.validate()?;
        let contents = to_json(board)?;
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }

        let mut file = AtomicWriteFile::open(&self.path)
            .map_err(|error| StoreError::Write(error.to_string()))?;
        file.write_all(&contents)?;
        file.sync_all()?;
        file.commit()
            .map_err(|error| StoreError::Write(error.to_string()))?;
        Ok(Fingerprint::of(&contents))
    }

    /// Writes the board only if the file still is what was last read or
    /// written here (`expected`, or `None` if there was no file), so that
    /// changes made by another program aren't lost. A file that has since
    /// been deleted is written again.
    pub fn save_unless_changed(
        &self,
        board: &Board,
        expected: Option<Fingerprint>,
    ) -> Result<Fingerprint, StoreError> {
        match self.fingerprint()? {
            Some(current) if Some(current) != expected => Err(StoreError::Conflict),
            _ => self.save(board),
        }
    }
}

/// The board as saved: pretty-printed with fields in a fixed order, so
/// each task is one block and diffs stay small.
pub fn to_json(board: &Board) -> Result<Vec<u8>, StoreError> {
    let mut contents = serde_json::to_vec_pretty(board)?;
    contents.push(b'\n');
    Ok(contents)
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
    fn unknown_fields_survive_a_load_and_save() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("board.json");
        fs::write(
            &path,
            r#"{
  "schema_version": 1,
  "name": "Mine",
  "owner": "sam",
  "columns": [
    {
      "id": "todo",
      "name": "To do",
      "wip_limit": 3,
      "tasks": [
        {
          "id": "00000000-0000-4000-8000-000000000001",
          "title": "Keep me",
          "created_at": 1,
          "updated_at": 2,
          "priority": "high",
          "tags": ["a", "b"]
        }
      ]
    }
  ]
}
"#,
        )
        .unwrap();
        let store = JsonStore::new(&path);
        let mut board = store.load().unwrap().unwrap();
        assert_eq!(board.extra["owner"], "sam");
        assert_eq!(board.columns[0].extra["wip_limit"], 3);
        let id = board.columns[0].tasks[0].id;
        board.update_task(id, "Kept", "", 5).unwrap();
        board.move_task(id, 0, Some(0), 6).unwrap();
        store.save(&board).unwrap();
        // Unknown fields follow the known ones, sorted by name.
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            r#"{
  "schema_version": 1,
  "name": "Mine",
  "columns": [
    {
      "id": "todo",
      "name": "To do",
      "tasks": [
        {
          "id": "00000000-0000-4000-8000-000000000001",
          "title": "Kept",
          "description": "",
          "created_at": 1,
          "updated_at": 6,
          "priority": "high",
          "tags": [
            "a",
            "b"
          ]
        }
      ],
      "wip_limit": 3
    }
  ],
  "owner": "sam"
}
"#
        );
    }

    #[test]
    fn saving_twice_gives_the_same_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("board.json");
        let store = JsonStore::new(&path);
        let mut board = Board::default();
        board.add_task(0, "Stable", "", 0).unwrap();
        let first = store.save(&board).unwrap();
        let bytes = fs::read(&path).unwrap();
        let loaded = store.read().unwrap().unwrap();
        assert_eq!(loaded.fingerprint, first);
        assert_eq!(store.save(&loaded.board).unwrap(), first);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn saving_refuses_to_overwrite_a_file_changed_elsewhere() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("board.json");
        let store = JsonStore::new(&path);
        let board = Board::default();
        // No file yet: the first save creates it.
        let saved = store.save_unless_changed(&board, None).unwrap();
        let saved = store.save_unless_changed(&board, Some(saved)).unwrap();
        fs::write(&path, "{}").unwrap();
        assert!(matches!(
            store.save_unless_changed(&board, Some(saved)),
            Err(StoreError::Conflict)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
        // A file that appeared since loading is someone else's, too.
        assert!(matches!(
            store.save_unless_changed(&board, None),
            Err(StoreError::Conflict)
        ));
        // A deleted file is written again.
        fs::remove_file(&path).unwrap();
        store.save_unless_changed(&board, Some(saved)).unwrap();
        assert_eq!(store.load().unwrap().unwrap(), board);
    }

    fn rename_title(value: &mut serde_json::Value) -> Result<(), String> {
        let board = value.as_object_mut().ok_or("not an object")?;
        let title = board.remove("title").ok_or("no title")?;
        board.insert("name".to_owned(), title);
        Ok(())
    }

    #[test]
    fn older_files_are_backed_up_then_upgraded() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("board.json");
        let old = r#"{"schema_version": 0, "title": "Old", "columns": [{"id": "a", "name": "A", "tasks": []}]}"#;
        fs::write(&path, old).unwrap();
        let store = JsonStore {
            migrations: &[Migration {
                from: 0,
                apply: rename_title,
            }],
            ..JsonStore::new(&path)
        };
        let board = store.load().unwrap().unwrap();
        assert_eq!((board.schema_version, board.name.as_str()), (1, "Old"));
        let backup = directory.path().join("board.json.bak-v0");
        assert_eq!(store.backup_path(0), backup);
        assert_eq!(fs::read_to_string(&backup).unwrap(), old);
        // The file itself is only rewritten by the next save, and a second
        // load keeps the first backup.
        assert_eq!(fs::read_to_string(&path).unwrap(), old);
        fs::write(&path, old.replace("Old", "Changed")).unwrap();
        store.load().unwrap();
        assert_eq!(fs::read_to_string(&backup).unwrap(), old);
    }

    #[test]
    fn files_from_newer_versions_are_refused() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("board.json");
        fs::write(&path, r#"{"schema_version": 99, "name": "Future"}"#).unwrap();
        let error = JsonStore::new(&path).load().unwrap_err();
        assert!(error.to_string().contains("newer"), "{error}");
        assert!(!directory.path().join("board.json.bak-v99").exists());
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
