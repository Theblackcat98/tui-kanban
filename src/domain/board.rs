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
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Column {
    pub id: String,
    pub name: String,
    pub tasks: Vec<Task>,
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
                Column::new("backlog", "Backlog"),
                Column::new("in-progress", "In Progress"),
                Column::new("done", "Done"),
            ],
        }
    }
}

impl Column {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            tasks: Vec::new(),
        }
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

    pub fn column_index(&self, id: &str) -> Option<usize> {
        self.columns.iter().position(|column| column.id == id)
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

    pub fn column(&self, index: usize) -> Result<&Column, BoardError> {
        self.columns
            .get(index)
            .ok_or(BoardError::InvalidColumn(index))
    }

    pub fn add_task(
        &mut self,
        column_index: usize,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<Uuid, BoardError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(BoardError::EmptyTitle);
        }
        let column = self
            .columns
            .get_mut(column_index)
            .ok_or(BoardError::InvalidColumn(column_index))?;
        let task = Task::new(title.trim().to_owned(), description.into());
        let id = task.id;
        column.tasks.push(task);
        Ok(id)
    }

    pub fn update_task(
        &mut self,
        id: Uuid,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<(), BoardError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(BoardError::EmptyTitle);
        }
        let (column_index, task_index) =
            self.task_location(id).ok_or(BoardError::TaskNotFound(id))?;
        self.columns[column_index].tasks[task_index]
            .update(title.trim().to_owned(), description.into());
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
        self.columns[to_column]
            .tasks
            .insert(insertion_index, task.clone());
        self.columns[to_column].tasks[insertion_index].touch();

        Ok(MoveOutcome {
            task_id: id,
            from_column,
            from_index,
            to_column,
            to_index: insertion_index,
        })
    }

    pub fn search(&self, query: &str) -> usize {
        self.columns
            .iter()
            .flat_map(|column| column.tasks.iter())
            .filter(|task| task.matches(query))
            .count()
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
    fn task_lifecycle_updates_board() {
        let mut board = Board::default();
        let id = board
            .add_task(0, "Write parser", "Keep the format readable")
            .unwrap();
        assert_eq!(board.task_count(), 1);
        assert_eq!(board.task(id).unwrap().title, "Write parser");

        board
            .update_task(id, "Write UI", "Ship the first screen")
            .unwrap();
        assert_eq!(board.task(id).unwrap().title, "Write UI");

        let outcome = board.move_task(id, 1, None).unwrap();
        assert_eq!(outcome.from_column, 0);
        assert_eq!(outcome.to_column, 1);
        assert_eq!(board.task(id).unwrap().title, "Write UI");

        board.remove_task(id).unwrap();
        assert!(board.task(id).is_none());
    }

    #[test]
    fn same_column_move_uses_final_index() {
        let mut board = Board::default();
        let first = board.add_task(0, "First", "").unwrap();
        board.add_task(0, "Second", "").unwrap();
        board.add_task(0, "Third", "").unwrap();
        board.move_task(first, 0, Some(2)).unwrap();
        assert_eq!(board.columns[0].tasks[1].id, first);
    }

    #[test]
    fn invalid_tasks_are_rejected() {
        let mut board = Board::default();
        assert_eq!(board.add_task(0, "  ", ""), Err(BoardError::EmptyTitle));
        assert_eq!(
            board.add_task(9, "Task", ""),
            Err(BoardError::InvalidColumn(9))
        );
    }

    #[test]
    fn search_matches_title_and_description() {
        let mut board = Board::default();
        board.add_task(0, "Design cards", "Use Catppuccin").unwrap();
        board.add_task(0, "Write tests", "Cover storage").unwrap();
        assert_eq!(board.search("catppuccin"), 1);
        assert_eq!(board.search("storage"), 1);
        assert_eq!(board.search(""), 2);
    }
}
