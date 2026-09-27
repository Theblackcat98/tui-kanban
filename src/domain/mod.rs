pub mod board;
pub mod filter;
pub mod task;

pub use board::{Board, BoardError, Column, MoveOutcome, SCHEMA_VERSION};
pub use filter::{Filter, Fuzzy, Term};
pub use task::Task;

/// Fields in the board file that this version doesn't know about, such as
/// ones added by hand or by a newer version. They are kept as they are and
/// written back, sorted by name, when the board is saved.
pub type Extra = serde_json::Map<String, serde_json::Value>;
