//! Which board to open: the project's board, found by searching upwards
//! from the current directory the way git finds `.git`; a board named on
//! the command line, by path or by name; or a personal board in the data
//! directory. Also the list of recently opened boards that the board
//! switcher shows.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// A project's board file.
pub const FILE_NAME: &str = ".tui-kanban.json";
/// The personal board's name, in the data directory.
pub const PERSONAL: &str = "personal";
/// How many recent boards are remembered.
pub const RECENT_LIMIT: usize = 20;

/// The nearest board file in `start` or a directory above it. The search
/// stops at the git root (the first directory holding `.git`), at `home`,
/// or at the top of the file system, whichever comes first.
pub fn find_upward(start: &Path, home: Option<&Path>) -> Option<PathBuf> {
    for directory in start.ancestors() {
        let candidate = directory.join(FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if directory.join(".git").exists() || Some(directory) == home {
            break;
        }
    }
    None
}

/// Where the board called `name` lives in the data directory.
pub fn named(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join("boards").join(format!("{name}.json"))
}

/// What `--board` names: a path, or the name of a board.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Named {
    Path(PathBuf),
    Name(String),
}

impl Named {
    /// A value with a path separator, a `.json` ending or a leading dot,
    /// or one naming an existing file, is a path; anything else is a name.
    pub fn parse(value: &str) -> Self {
        let looks_like_path = value.contains(['/', '\\'])
            || value.ends_with(".json")
            || value.starts_with('.')
            || Path::new(value).is_file();
        if looks_like_path {
            Self::Path(PathBuf::from(value))
        } else {
            Self::Name(value.to_owned())
        }
    }
}

/// Finds the board called `name`: a recent board whose name (in the file)
/// or file name matches, ignoring case, or else the personal board of that
/// name in the data directory, which is created when first changed.
pub fn resolve_name(name: &str, recent: &[PathBuf], data_dir: Option<&Path>) -> Option<PathBuf> {
    let wanted = name.to_lowercase();
    let recent_match = recent.iter().find(|path| {
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase());
        stem.as_deref() == Some(wanted.as_str())
            || summary(path).is_some_and(|summary| summary.name.to_lowercase() == wanted)
    });
    recent_match
        .cloned()
        .or_else(|| data_dir.map(|dir| named(dir, name)))
}

/// A name for a new board at `path`: the project directory's name for a
/// project board, or the file's name.
pub fn default_name(path: &Path) -> String {
    let file_name = path.file_name().map(|name| name.to_string_lossy());
    if file_name.as_deref() == Some(FILE_NAME) {
        let directory = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let directory = directory
            .canonicalize()
            .unwrap_or_else(|_| directory.to_owned());
        if let Some(name) = directory.file_name() {
            return name.to_string_lossy().into_owned();
        }
    }
    match path.file_stem() {
        Some(stem) if stem == PERSONAL => "Personal".to_owned(),
        Some(stem) => stem.to_string_lossy().into_owned(),
        None => "Board".to_owned(),
    }
}

/// What the board switcher shows about a board file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Summary {
    pub name: String,
    pub tasks: usize,
}

/// A board's name and task count, read without upgrading or checking the
/// file, or `None` if it can't be read.
pub fn summary(path: &Path) -> Option<Summary> {
    let text = fs::read(path).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&text).ok()?;
    let name = value.get("name")?.as_str()?.to_owned();
    let tasks = value
        .get("columns")?
        .as_array()?
        .iter()
        .filter_map(|column| column.get("tasks")?.as_array().map(Vec::len))
        .sum();
    Some(Summary { name, tasks })
}

/// The recently opened boards, most recent first, kept one path per line
/// in the state directory.
#[derive(Clone, Debug)]
pub struct Recent {
    file: PathBuf,
}

impl Recent {
    pub fn new(file: impl Into<PathBuf>) -> Self {
        Self { file: file.into() }
    }

    pub fn load(&self) -> Vec<PathBuf> {
        fs::read_to_string(&self.file)
            .map(|text| {
                text.lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Puts `path` first, as an absolute path. Best effort: if the list
    /// can't be written, the board just isn't remembered.
    pub fn record(&self, path: &Path) {
        let path = absolute(path);
        let mut paths = self.load();
        paths.retain(|other| *other != path);
        paths.insert(0, path);
        paths.truncate(RECENT_LIMIT);
        let text: String = paths
            .iter()
            .map(|path| format!("{}\n", path.display()))
            .collect();
        if let Some(parent) = self.file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&self.file, text);
    }
}

/// `path` made absolute, resolving links when the file exists.
pub fn absolute(path: &Path) -> PathBuf {
    match path.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            std::path::absolute(path).unwrap_or_else(|_| path.to_owned())
        }
        Err(_) => path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_board_is_found_upwards_but_not_past_the_git_root() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        let deep = project.join("src").join("deep");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir(project.join(".git")).unwrap();
        assert_eq!(find_upward(&deep, None), None);
        // A board above the git root belongs to something else.
        fs::write(root.path().join(FILE_NAME), "{}").unwrap();
        assert_eq!(find_upward(&deep, None), None);
        fs::write(project.join(FILE_NAME), "{}").unwrap();
        assert_eq!(find_upward(&deep, None), Some(project.join(FILE_NAME)));
        fs::write(deep.join(FILE_NAME), "{}").unwrap();
        assert_eq!(find_upward(&deep, None), Some(deep.join(FILE_NAME)));
        // Outside a repository, the search stops at home.
        let home = root.path().join("home");
        let work = home.join("work");
        fs::create_dir_all(&work).unwrap();
        assert_eq!(find_upward(&work, Some(&home)), None);
        fs::write(home.join(FILE_NAME), "{}").unwrap();
        assert_eq!(find_upward(&work, Some(&home)), Some(home.join(FILE_NAME)));
    }

    #[test]
    fn board_values_are_paths_or_names() {
        assert_eq!(Named::parse("work"), Named::Name("work".to_owned()));
        for path in ["./work", "boards/work", "work.json", ".tui-kanban.json"] {
            assert_eq!(Named::parse(path), Named::Path(PathBuf::from(path)));
        }
    }

    #[test]
    fn names_find_recent_boards_or_personal_ones() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join(FILE_NAME);
        fs::write(
            &project,
            r#"{"schema_version": 1, "name": "Website", "columns": [
                {"id": "a", "name": "A", "tasks": [{}, {}]}, {"id": "b", "name": "B", "tasks": []}]}"#,
        )
        .unwrap();
        assert_eq!(
            summary(&project),
            Some(Summary {
                name: "Website".to_owned(),
                tasks: 2
            })
        );
        let data = directory.path().join("data");
        let recent = [project.clone()];
        assert_eq!(resolve_name("website", &recent, Some(&data)), Some(project));
        assert_eq!(
            resolve_name("work", &recent, Some(&data)),
            Some(data.join("boards").join("work.json"))
        );
        assert_eq!(resolve_name("work", &recent, None), None);
    }

    #[test]
    fn new_boards_are_named_after_their_place() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("my-project");
        fs::create_dir(&project).unwrap();
        assert_eq!(default_name(&project.join(FILE_NAME)), "my-project");
        assert_eq!(default_name(Path::new("/data/boards/work.json")), "work");
        assert_eq!(default_name(Path::new("personal.json")), "Personal");
    }

    #[test]
    fn recent_boards_are_most_recent_first() {
        let directory = tempfile::tempdir().unwrap();
        let recent = Recent::new(directory.path().join("state").join("recent-boards"));
        assert!(recent.load().is_empty());
        let first = directory.path().join("one.json");
        let second = directory.path().join("two.json");
        fs::write(&first, "{}").unwrap();
        recent.record(&first);
        recent.record(&second);
        recent.record(&first);
        let loaded = recent.load();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0], absolute(&first));
        assert_eq!(loaded[1], absolute(&second));
        for index in 0..RECENT_LIMIT + 5 {
            recent.record(&directory.path().join(format!("{index}.json")));
        }
        assert_eq!(recent.load().len(), RECENT_LIMIT);
    }
}
