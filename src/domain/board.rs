use crate::domain::Extra;
use crate::domain::task::Task;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Board {
    pub schema_version: u32,
    pub name: String,
    pub columns: Vec<Column>,
    /// Fields this version doesn't know, kept so saving doesn't drop them.
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Column {
    pub id: String,
    pub name: String,
    /// The column's accent: a palette name such as `"mauve"`, or
    /// `"#rrggbb"`. Without one, a colour is chosen from the id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub tasks: Vec<Task>,
    /// Fields this version doesn't know, kept so saving doesn't drop them.
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoveOutcome {
    pub task_id: Uuid,
    pub from_column: usize,
    pub from_index: usize,
    pub to_column: usize,
    pub to_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoardError {
    EmptyTitle,
    InvalidColumn(usize),
    TaskNotFound(Uuid),
    UnsupportedSchema(u32),
    InvalidBoard(String),
}

impl fmt::Display for BoardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => formatter.write_str("task title cannot be empty"),
            Self::InvalidColumn(index) => write!(formatter, "column index {index} does not exist"),
            Self::TaskNotFound(id) => write!(formatter, "task {id} does not exist"),
            Self::UnsupportedSchema(version) if *version > SCHEMA_VERSION => write!(
                formatter,
                "board schema version {version} is newer than this tui-kanban reads \
                 ({SCHEMA_VERSION}); upgrade tui-kanban to open it"
            ),
            Self::UnsupportedSchema(version) => {
                write!(formatter, "unsupported board schema version {version}")
            }
            Self::InvalidBoard(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for BoardError {}

impl Default for Board {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            name: "Project Board".to_owned(),
            columns: vec![
                Column::new("backlog", "Backlog").with_color("sapphire"),
                Column::new("in-progress", "In Progress").with_color("peach"),
                Column::new("done", "Done").with_color("green"),
            ],
            extra: Extra::new(),
        }
    }
}

impl Column {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            color: None,
            tasks: Vec::new(),
            extra: Extra::new(),
        }
    }

    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl Board {
    pub fn validate(&self) -> Result<(), BoardError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(BoardError::UnsupportedSchema(self.schema_version));
        }
        if self.name.trim().is_empty() {
            return Err(BoardError::InvalidBoard(
                "board name cannot be empty".to_owned(),
            ));
        }
        if self.columns.is_empty() {
            return Err(BoardError::InvalidBoard(
                "board must contain a column".to_owned(),
            ));
        }

        let mut column_ids = HashSet::new();
        let mut task_ids = HashSet::new();
        for column in &self.columns {
            if column.id.trim().is_empty() || column.name.trim().is_empty() {
                return Err(BoardError::InvalidBoard(
                    "column ids and names cannot be empty".to_owned(),
                ));
            }
            if !column_ids.insert(column.id.as_str()) {
                return Err(BoardError::InvalidBoard(format!(
                    "duplicate column id: {}",
                    column.id
                )));
            }
            for task in &column.tasks {
                if task.title.trim().is_empty() {
                    return Err(BoardError::InvalidBoard(format!(
                        "task {} has an empty title",
                        task.id
                    )));
                }
                if !task_ids.insert(task.id) {
                    return Err(BoardError::InvalidBoard(format!(
                        "duplicate task id: {}",
                        task.id
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn task_location(&self, id: Uuid) -> Option<(usize, usize)> {
        self.columns
            .iter()
            .enumerate()
            .find_map(|(column_index, column)| {
                column
                    .tasks
                    .iter()
                    .position(|task| task.id == id)
                    .map(|task_index| (column_index, task_index))
            })
    }

    pub fn task(&self, id: Uuid) -> Option<&Task> {
        self.task_location(id)
            .and_then(|(column_index, task_index)| self.columns[column_index].tasks.get(task_index))
    }

    /// Adds a task to the end of a column.
    pub fn add_task(
        &mut self,
        column_index: usize,
        title: impl Into<String>,
        description: impl Into<String>,
        now: i64,
    ) -> Result<Uuid, BoardError> {
        self.insert_task(column_index, usize::MAX, title, description, now)
    }

    /// Adds a task at `index` in a column, or at the end if `index` is
    /// past it.
    pub fn insert_task(
        &mut self,
        column_index: usize,
        index: usize,
        title: impl Into<String>,
        description: impl Into<String>,
        now: i64,
    ) -> Result<Uuid, BoardError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(BoardError::EmptyTitle);
        }
        let column = self
            .columns
            .get_mut(column_index)
            .ok_or(BoardError::InvalidColumn(column_index))?;
        let task = Task::new(title.trim().to_owned(), description.into(), now);
        let id = task.id;
        column.tasks.insert(index.min(column.tasks.len()), task);
        Ok(id)
    }

    /// Adds a copy of a task, with a new id and times, right below it.
    pub fn duplicate_task(&mut self, id: Uuid, now: i64) -> Result<Uuid, BoardError> {
        let (column, index) = self.task_location(id).ok_or(BoardError::TaskNotFound(id))?;
        let original = &self.columns[column].tasks[index];
        let copy = Task::new(original.title.clone(), original.description.clone(), now);
        let copy_id = copy.id;
        self.columns[column].tasks.insert(index + 1, copy);
        Ok(copy_id)
    }

    pub fn update_task(
        &mut self,
        id: Uuid,
        title: impl Into<String>,
        description: impl Into<String>,
        now: i64,
    ) -> Result<(), BoardError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(BoardError::EmptyTitle);
        }
        let (column_index, task_index) =
            self.task_location(id).ok_or(BoardError::TaskNotFound(id))?;
        self.columns[column_index].tasks[task_index].update(
            title.trim().to_owned(),
            description.into(),
            now,
        );
        Ok(())
    }

    pub fn remove_task(&mut self, id: Uuid) -> Result<Task, BoardError> {
        let (column_index, task_index) =
            self.task_location(id).ok_or(BoardError::TaskNotFound(id))?;
        Ok(self.columns[column_index].tasks.remove(task_index))
    }

    pub fn move_task(
        &mut self,
        id: Uuid,
        to_column: usize,
        to_index: Option<usize>,
        now: i64,
    ) -> Result<MoveOutcome, BoardError> {
        let (from_column, from_index) =
            self.task_location(id).ok_or(BoardError::TaskNotFound(id))?;
        if to_column >= self.columns.len() {
            return Err(BoardError::InvalidColumn(to_column));
        }

        let same_column = from_column == to_column;
        let task = self.columns[from_column].tasks.remove(from_index);
        let max_index = self.columns[to_column].tasks.len();
        let requested_index = to_index.unwrap_or(max_index);
        let insertion_index = if same_column && from_index < requested_index {
            requested_index - 1
        } else {
            requested_index
        }
        .min(max_index);
        self.columns[to_column].tasks.insert(insertion_index, task);
        self.columns[to_column].tasks[insertion_index].touch(now);

        Ok(MoveOutcome {
            task_id: id,
            from_column,
            from_index,
            to_column,
            to_index: insertion_index,
        })
    }

    pub fn task_count(&self) -> usize {
        self.columns.iter().map(|column| column.tasks.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_board_has_three_columns() {
        let board = Board::default();
        assert_eq!(board.schema_version, SCHEMA_VERSION);
        assert_eq!(board.columns.len(), 3);
        assert_eq!(board.columns[1].id, "in-progress");
        assert!(board.task_count() == 0);
    }

    #[test]
    fn column_colour_is_optional_in_the_file() {
        // Files without colours still load, and columns without one are
        // saved without the field.
        let column: Column =
            serde_json::from_str(r#"{"id": "a", "name": "A", "tasks": []}"#).unwrap();
        assert_eq!(column.color, None);
        assert!(!serde_json::to_string(&column).unwrap().contains("color"));
        let column = Column::new("a", "A").with_color("teal");
        let json = serde_json::to_string(&column).unwrap();
        assert!(json.contains(r#""color":"teal""#));
        assert_eq!(serde_json::from_str::<Column>(&json).unwrap(), column);
    }

    #[test]
    fn task_lifecycle_updates_board() {
        let mut board = Board::default();
        let id = board
            .add_task(0, "Write parser", "Keep the format readable", 0)
            .unwrap();
        assert_eq!(board.task_count(), 1);
        assert_eq!(board.task(id).unwrap().title, "Write parser");

        board
            .update_task(id, "Write UI", "Ship the first screen", 0)
            .unwrap();
        assert_eq!(board.task(id).unwrap().title, "Write UI");

        let outcome = board.move_task(id, 1, None, 0).unwrap();
        assert_eq!(outcome.from_column, 0);
        assert_eq!(outcome.to_column, 1);
        assert_eq!(board.task(id).unwrap().title, "Write UI");

        board.remove_task(id).unwrap();
        assert!(board.task(id).is_none());
    }

    #[test]
    fn tasks_can_be_inserted_and_duplicated_in_place() {
        let mut board = Board::default();
        let last = board.add_task(0, "Last", "", 0).unwrap();
        let first = board.insert_task(0, 0, "First", "", 0).unwrap();
        let copy = board.duplicate_task(first, 5).unwrap();
        let ids: Vec<Uuid> = board.columns[0].tasks.iter().map(|task| task.id).collect();
        assert_eq!(ids, [first, copy, last]);
        let copied = board.task(copy).unwrap();
        assert_eq!(copied.title, "First");
        assert_eq!(copied.created_at, 5);
        assert_eq!(
            board.duplicate_task(Uuid::nil(), 0),
            Err(BoardError::TaskNotFound(Uuid::nil()))
        );
    }

    #[test]
    fn same_column_move_uses_final_index() {
        let mut board = Board::default();
        let first = board.add_task(0, "First", "", 0).unwrap();
        board.add_task(0, "Second", "", 0).unwrap();
        board.add_task(0, "Third", "", 0).unwrap();
        board.move_task(first, 0, Some(2), 0).unwrap();
        assert_eq!(board.columns[0].tasks[1].id, first);
    }

    #[test]
    fn invalid_tasks_are_rejected() {
        let mut board = Board::default();
        assert_eq!(board.add_task(0, "  ", "", 0), Err(BoardError::EmptyTitle));
        assert_eq!(
            board.add_task(9, "Task", "", 0),
            Err(BoardError::InvalidColumn(9))
        );
    }
}
