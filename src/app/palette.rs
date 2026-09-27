//! The command palette: every command available on the screen it was
//! opened from, plus "go to lane" and "go to task", fuzzy-searched, with
//! recently used commands first.

use uuid::Uuid;

use super::input::TextInput;
use super::model::Model;
use crate::command::{self, CommandId, Context, Group};
use crate::domain::Fuzzy;

/// How many recently run commands the palette remembers.
pub const RECENT_LIMIT: usize = 8;

#[derive(Clone, Debug)]
pub struct Palette {
    pub input: TextInput,
    /// The highlighted entry.
    pub selected: usize,
    /// The context the palette was opened from, whose commands it lists.
    pub context: Context,
}

impl Palette {
    pub fn new(context: Context) -> Self {
        Self {
            input: TextInput::new(""),
            selected: 0,
            context,
        }
    }
}

/// What an entry does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Command(CommandId),
    Lane(usize),
    Task(Uuid),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub label: String,
    /// The shortcut, shown on the right.
    pub keys: String,
    /// Where a task is, for "go to task" entries.
    pub detail: String,
    pub target: Target,
    /// The graphemes of `label` the query matched.
    pub highlights: Vec<usize>,
}

/// Commands that don't make sense from the palette: ones that need a key
/// as their argument, and ways of opening menus of keys.
fn in_palette(id: CommandId) -> bool {
    !matches!(
        id,
        CommandId::OpenPalette
            | CommandId::Leader
            | CommandId::JumpToLane
            | CommandId::GoToLane
            | CommandId::ForceQuit
    )
}

fn capitalised(label: &str) -> String {
    let mut characters = label.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

/// The palette's entries for `query`, best first.
pub fn entries(model: &Model, palette: &Palette) -> Vec<Entry> {
    let recent = &model.session.recent_commands;
    let mut commands: Vec<&command::Command> = command::available(palette.context)
        .filter(|command| command.listed && in_palette(command.id))
        .filter(|command| model.command_enabled(command.id))
        .collect();
    // Recently used first, most recent at the top; then actions before
    // movement, each group in table order.
    commands.sort_by_key(|command| {
        let recency = recent
            .iter()
            .position(|id| *id == command.id)
            .unwrap_or(usize::MAX);
        let group = match command.group {
            Group::Tasks => 0,
            Group::Columns => 1,
            Group::Details => 2,
            Group::Navigation => 3,
            Group::General => 4,
            Group::Editing => 5,
        };
        (recency, group)
    });
    let mut entries: Vec<Entry> = commands
        .into_iter()
        .map(|command| Entry {
            label: capitalised(command.label),
            keys: command
                .shown_keys
                .map(str::to_owned)
                .or_else(|| command.keys.first().map(|key| key.label()))
                .unwrap_or_default(),
            detail: String::new(),
            target: Target::Command(command.id),
            highlights: Vec::new(),
        })
        .collect();
    for (index, column) in model.board.columns.iter().enumerate() {
        entries.push(Entry {
            label: format!("Go to lane: {}", column.name),
            keys: if index < 9 {
                (index + 1).to_string()
            } else {
                String::new()
            },
            detail: String::new(),
            target: Target::Lane(index),
            highlights: Vec::new(),
        });
    }
    for column in &model.board.columns {
        for task in &column.tasks {
            entries.push(Entry {
                label: format!("Go to task: {}", task.title),
                keys: String::new(),
                detail: column.name.clone(),
                target: Target::Task(task.id),
                highlights: Vec::new(),
            });
        }
    }

    let query = palette.input.value.trim();
    if query.is_empty() {
        return entries;
    }
    let fuzzy = Fuzzy::new(query);
    let mut scored: Vec<(u32, Entry)> = entries
        .into_iter()
        .filter_map(|mut entry| {
            let (score, highlights) = fuzzy.score(&entry.label)?;
            entry.highlights = highlights;
            Some((score, entry))
        })
        .collect();
    // A stable sort keeps recent commands ahead of equally good matches.
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(_, entry)| entry).collect()
}

/// Puts `id` at the front of the recent commands.
pub fn remember(recent: &mut Vec<CommandId>, id: CommandId) {
    recent.retain(|other| *other != id);
    recent.insert(0, id);
    recent.truncate(RECENT_LIMIT);
}
