use serde::{Deserialize, Serialize};
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
    pub fn new(title: impl Into<String>, description: impl Into<String>, now: i64) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            description: description.into(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update(&mut self, title: impl Into<String>, description: impl Into<String>, now: i64) {
        self.title = title.into();
        self.description = description.into();
        self.updated_at = now;
    }

    pub fn touch(&mut self, now: i64) {
        self.updated_at = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_updates_timestamp_and_content() {
        let mut task = Task::new("Initial", "Description", 1_000);
        assert_eq!(task.created_at, 1_000);
        task.update("Changed", "New description", 2_000);
        assert_eq!(task.title, "Changed");
        assert_eq!(task.description, "New description");
        assert_eq!(task.created_at, 1_000);
        assert_eq!(task.updated_at, 2_000);
    }
}
