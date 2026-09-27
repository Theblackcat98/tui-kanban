//! Upgrading board files written by older versions of tui-kanban.
//!
//! Each schema change adds a step to [`MIGRATIONS`] and bumps
//! [`SCHEMA_VERSION`](crate::domain::SCHEMA_VERSION). Loading an older file runs the steps in order, after
//! [`super::JsonStore`] has backed the file up as `<file>.bak-v<version>`.
//! The file itself is only rewritten by the next save.

use serde_json::Value;

use crate::domain::BoardError;

/// One step: upgrades a board, as JSON, from `from` to `from + 1`. The
/// caller updates `schema_version` afterwards.
#[derive(Clone, Copy, Debug)]
pub struct Migration {
    pub from: u32,
    pub apply: fn(&mut Value) -> Result<(), String>,
}

/// Every step, oldest first. Empty while the schema is at its first
/// version.
pub const MIGRATIONS: &[Migration] = &[];

/// The file's `schema_version`.
pub fn version_of(value: &Value) -> Result<u32, BoardError> {
    value
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .ok_or_else(|| BoardError::InvalidBoard("missing or invalid schema_version".to_owned()))
}

/// Upgrades `value` from `version` to `target`, one step at a time.
pub fn upgrade(
    mut value: Value,
    mut version: u32,
    target: u32,
    steps: &[Migration],
) -> Result<Value, BoardError> {
    if version > target {
        return Err(BoardError::UnsupportedSchema(version));
    }
    while version < target {
        let step = steps
            .iter()
            .find(|step| step.from == version)
            .ok_or(BoardError::UnsupportedSchema(version))?;
        (step.apply)(&mut value).map_err(|message| {
            BoardError::InvalidBoard(format!(
                "could not upgrade from schema version {version}: {message}"
            ))
        })?;
        version += 1;
        value["schema_version"] = Value::from(version);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SCHEMA_VERSION;
    use serde_json::json;

    fn rename_title(value: &mut Value) -> Result<(), String> {
        let name = value
            .as_object_mut()
            .and_then(|board| board.remove("title"))
            .ok_or("no title")?;
        value["name"] = name;
        Ok(())
    }

    fn add_columns(value: &mut Value) -> Result<(), String> {
        value["columns"] = json!([]);
        Ok(())
    }

    const STEPS: &[Migration] = &[
        Migration {
            from: 1,
            apply: add_columns,
        },
        Migration {
            from: 0,
            apply: rename_title,
        },
    ];

    #[test]
    fn steps_run_in_version_order() {
        let old = json!({"schema_version": 0, "title": "Old"});
        let new = upgrade(old, 0, 2, STEPS).unwrap();
        assert_eq!(
            new,
            json!({"schema_version": 2, "name": "Old", "columns": []})
        );
    }

    #[test]
    fn a_failing_step_names_its_version() {
        let old = json!({"schema_version": 0});
        let error = upgrade(old, 0, 2, STEPS).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("from schema version 0: no title")
        );
    }

    #[test]
    fn newer_and_unknown_versions_are_refused() {
        let value = json!({"schema_version": 7});
        assert_eq!(version_of(&value), Ok(7));
        assert_eq!(
            upgrade(value, 7, 2, STEPS),
            Err(BoardError::UnsupportedSchema(7))
        );
        assert_eq!(
            upgrade(json!({}), 5, 9, STEPS),
            Err(BoardError::UnsupportedSchema(5))
        );
        assert!(version_of(&json!({"schema_version": "1"})).is_err());
    }

    #[test]
    fn every_older_version_has_one_step() {
        let mut from: Vec<u32> = MIGRATIONS.iter().map(|step| step.from).collect();
        from.sort_unstable();
        let older: Vec<u32> = (1..)
            .take_while(|version| *version < SCHEMA_VERSION)
            .collect();
        assert_eq!(from, older);
    }
}
