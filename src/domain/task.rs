use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Task {
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        let now = now_millis();
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            description: description.into(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update(&mut self, title: impl Into<String>, description: impl Into<String>) {
        self.title = title.into();
        self.description = description.into();
        self.updated_at = now_millis();
    }

    pub fn touch(&mut self) {
        self.updated_at = now_millis();
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.title.to_lowercase().contains(&query)
            || self.description.to_lowercase().contains(&query)
    }
}

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_updates_timestamp_and_content() {
        let mut task = Task::new("Initial", "Description");
        let original = task.updated_at;
        task.update("Changed", "New description");
        assert_eq!(task.title, "Changed");
        assert_eq!(task.description, "New description");
        assert!(task.updated_at >= original);
    }

    #[test]
    fn matching_is_case_insensitive() {
        let task = Task::new("Build UI", "Catppuccin colors");
        assert!(task.matches("catppuccin"));
        assert!(task.matches("UI"));
        assert!(!task.matches("database"));
        assert!(task.matches(""));
    }

    #[test]
    fn timestamp_is_non_negative() {
        assert!(now_millis() >= 0);
    }
}
