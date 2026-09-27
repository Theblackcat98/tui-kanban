//! Search filters: free text matched fuzzily, plus filter terms.
//!
//! ```text
//! in:progress #ui updated:<7d readme
//! ```
//!
//! - `in:name` keeps tasks in columns whose name (or id) contains `name`,
//!   ignoring case, spaces and punctuation.
//! - `#tag` keeps tasks whose title or description contains the hashtag.
//! - `updated:` and `created:` take `today`, `<7d` (within the last seven
//!   days), `>2w` (longer ago than two weeks) or a plain `3d` (within);
//!   the units are `h`, `d` and `w`.
//! - Any other word is text. It matches a task's title fuzzily (the
//!   letters in order, with gaps, via `nucleo-matcher`, ignoring case and
//!   accents) or its description as a substring. fzf's syntax works too:
//!   `'word` for an exact substring, `^word` / `word$` for a prefix or
//!   suffix, and `!word` to exclude.
//!
//! Every term must match. Times are resolved against the clock when the
//! filter is parsed, so matching is a pure function of the task.

use std::cell::RefCell;

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use unicode_segmentation::UnicodeSegmentation;

use super::{Column, Task};

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

/// A fuzzy title match must average this score per query character, so
/// that short queries don't match letters scattered across long titles.
/// A match at word starts scores about 30 per character; letters spread
/// through unrelated words score under 20.
const MIN_SCORE_PER_CHAR: u32 = 20;

/// One term of a filter, as typed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Term {
    /// `in:name`: the column, normalised to lowercase letters and digits.
    Column(String),
    /// `#tag`, lowercased, without the `#`.
    Tag(String),
    /// `updated:…`: a range of Unix milliseconds.
    Updated(TimeRange),
    /// `created:…`.
    Created(TimeRange),
    /// Anything else.
    Text(String),
}

/// Unix milliseconds from `after` (inclusive) to `before` (exclusive).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeRange {
    pub after: i64,
    pub before: i64,
}

impl TimeRange {
    fn contains(self, timestamp: i64) -> bool {
        (self.after..self.before).contains(&timestamp)
    }
}

/// A parsed search query.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    /// Each term with the text it was parsed from, in order.
    terms: Vec<(String, Term)>,
    /// The text terms, for matching the title.
    title_atoms: Vec<Atom>,
    /// The text terms, for matching the description.
    description_atoms: Vec<Atom>,
}

thread_local! {
    // Creating a matcher allocates its scratch space, so share one.
    static MATCHER: RefCell<Matcher> = RefCell::new(Matcher::new(Config::DEFAULT));
}

impl Filter {
    /// Parses a query, resolving times against `now` (Unix milliseconds).
    pub fn parse(query: &str, now: i64) -> Self {
        let mut filter = Self::default();
        for word in query.split_whitespace() {
            let term = parse_term(word, now);
            if let Term::Text(text) = &term {
                let atom = Atom::parse(text, CaseMatching::Smart, Normalization::Smart);
                // A plain word is fuzzy in titles, but must appear as
                // written in descriptions, which are long enough to
                // contain almost any letters in order.
                let description = if atom.kind == AtomKind::Fuzzy {
                    Atom::new(
                        text,
                        CaseMatching::Smart,
                        Normalization::Smart,
                        AtomKind::Substring,
                        false,
                    )
                } else {
                    atom.clone()
                };
                filter.title_atoms.push(atom);
                filter.description_atoms.push(description);
            }
            filter.terms.push((word.to_owned(), term));
        }
        filter
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// The terms as typed, for the filter bar.
    pub fn terms(&self) -> impl Iterator<Item = (&str, &Term)> {
        self.terms.iter().map(|(text, term)| (text.as_str(), term))
    }

    /// Whether a task in `column` passes every term.
    pub fn matches(&self, task: &Task, column: &Column) -> bool {
        let terms_match = self.terms.iter().all(|(_, term)| match term {
            Term::Column(name) => {
                normalise(&column.name).contains(name.as_str())
                    || normalise(&column.id).contains(name.as_str())
            }
            Term::Tag(tag) => has_tag(&task.title, tag) || has_tag(&task.description, tag),
            Term::Updated(range) => range.contains(task.updated_at),
            Term::Created(range) => range.contains(task.created_at),
            Term::Text(_) => true,
        });
        terms_match && self.text_matches(task)
    }

    fn text_matches(&self, task: &Task) -> bool {
        MATCHER.with_borrow_mut(|matcher| {
            let mut title = Haystack::default();
            let mut description = Haystack::default();
            let title = title.of(&task.title);
            let description = description.of(&task.description);
            self.title_atoms.iter().zip(&self.description_atoms).all(
                |(title_atom, description_atom)| {
                    let in_title = title_atom
                        .score(title, matcher)
                        .is_some_and(|score| good_enough(title_atom, score));
                    let in_description = description_atom.score(description, matcher).is_some();
                    if title_atom.negative {
                        // Excluded if it appears in either.
                        in_title && in_description
                    } else {
                        in_title || in_description
                    }
                },
            )
        })
    }

    /// Which graphemes of a title the text terms matched, for
    /// highlighting.
    pub fn title_highlights(&self, title: &str) -> Vec<usize> {
        highlights(&self.title_atoms, title, true)
    }

    /// Which graphemes of a line of description the text terms matched.
    pub fn description_highlights(&self, line: &str) -> Vec<usize> {
        highlights(&self.description_atoms, line, false)
    }
}

/// Room for a haystack: the text as one character per grapheme, so match
/// indices count graphemes.
#[derive(Default)]
struct Haystack {
    chars: Vec<char>,
    bytes: Vec<u8>,
}

impl Haystack {
    fn of<'a>(&'a mut self, text: &str) -> Utf32Str<'a> {
        self.chars.clear();
        self.chars.extend(
            text.graphemes(true)
                .map(|grapheme| grapheme.chars().next().unwrap_or(' ')),
        );
        // nucleo's substring matching expects ASCII text in its ASCII
        // form.
        if self.chars.iter().all(char::is_ascii) {
            self.bytes.clear();
            self.bytes
                .extend(self.chars.iter().map(|character| *character as u8));
            Utf32Str::Ascii(&self.bytes)
        } else {
            Utf32Str::Unicode(&self.chars)
        }
    }
}

fn good_enough(atom: &Atom, score: u16) -> bool {
    atom.negative
        || atom.kind != AtomKind::Fuzzy
        || u32::from(score) >= MIN_SCORE_PER_CHAR * atom.needle_text().len() as u32
}

fn highlights(atoms: &[Atom], text: &str, fuzzy: bool) -> Vec<usize> {
    MATCHER.with_borrow_mut(|matcher| {
        let mut storage = Haystack::default();
        let haystack = storage.of(text);
        let mut indices = Vec::new();
        for atom in atoms.iter().filter(|atom| !atom.negative) {
            let mut found = Vec::new();
            if let Some(score) = atom.indices(haystack, matcher, &mut found)
                && (!fuzzy || good_enough(atom, score))
            {
                indices.extend(found.into_iter().map(|index| index as usize));
            }
        }
        indices.sort_unstable();
        indices.dedup();
        indices
    })
}

fn parse_term(word: &str, now: i64) -> Term {
    let lower = word.to_lowercase();
    if let Some(name) = lower.strip_prefix("in:")
        && !name.is_empty()
    {
        return Term::Column(normalise(name));
    }
    if let Some(tag) = lower.strip_prefix('#')
        && !tag.is_empty()
        && tag.chars().all(is_tag_char)
    {
        return Term::Tag(tag.to_owned());
    }
    for (prefix, make) in [
        ("updated:", Term::Updated as fn(TimeRange) -> Term),
        ("created:", Term::Created),
    ] {
        if let Some(value) = lower.strip_prefix(prefix)
            && let Some(range) = parse_age(value, now)
        {
            return make(range);
        }
    }
    Term::Text(word.to_owned())
}

/// `today`, `yesterday`, `<7d`, `>2w` or `3h` into a time range.
fn parse_age(value: &str, now: i64) -> Option<TimeRange> {
    let midnight = now - now.rem_euclid(DAY);
    match value {
        "today" => {
            return Some(TimeRange {
                after: midnight,
                before: i64::MAX,
            });
        }
        "yesterday" => {
            return Some(TimeRange {
                after: midnight - DAY,
                before: midnight,
            });
        }
        _ => {}
    }
    let (older, amount) = match value.as_bytes().first()? {
        b'<' => (false, &value[1..]),
        b'>' => (true, &value[1..]),
        _ => (false, value),
    };
    let unit = match amount.chars().last()? {
        'h' => HOUR,
        'd' => DAY,
        'w' => 7 * DAY,
        _ => return None,
    };
    let count: i64 = amount[..amount.len() - 1].parse().ok()?;
    let cutoff = now.saturating_sub(count.saturating_mul(unit));
    Some(if older {
        TimeRange {
            after: i64::MIN,
            before: cutoff,
        }
    } else {
        TimeRange {
            after: cutoff,
            before: i64::MAX,
        }
    })
}

/// Lowercase letters and digits only, so `in:inprogress` finds
/// "In Progress" and `in:in-progress` does too.
fn normalise(text: &str) -> String {
    text.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_tag_char(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '-' | '_' | '/')
}

/// Whether `text` contains `#tag` as a whole tag, ignoring case.
fn has_tag(text: &str, tag: &str) -> bool {
    let lower = text.to_lowercase();
    lower.match_indices('#').any(|(index, _)| {
        let before_ok = lower[..index]
            .chars()
            .next_back()
            .is_none_or(|previous| !is_tag_char(previous));
        let rest = &lower[index + 1..];
        let after_ok = rest[tag.len().min(rest.len())..]
            .chars()
            .next()
            .is_none_or(|next| !is_tag_char(next));
        before_ok && rest.starts_with(tag) && after_ok
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_788_000_000_000; // 2026-08-29 10:40 UTC

    fn task(title: &str, description: &str, updated_ago: i64) -> Task {
        let mut task = Task::new(title, description, NOW - updated_ago - DAY);
        task.updated_at = NOW - updated_ago;
        task
    }

    fn matches(query: &str, task: &Task, column: &str) -> bool {
        Filter::parse(query, NOW).matches(task, &Column::new("col", column))
    }

    #[test]
    fn text_is_fuzzy_in_titles_and_exact_in_descriptions() {
        let readme = task("Write the README", "Cover install and usage", 0);
        assert!(matches("readme", &readme, "Backlog"));
        assert!(matches("wrt rdme", &readme, "Backlog"));
        assert!(matches("usage", &readme, "Backlog"));
        // Letters in order across the description don't count.
        assert!(!matches("cvrusg", &readme, "Backlog"));
        // Nor do letters scattered across a long title.
        let tiles = task("Tiles with an accent bar", "", 0);
        assert!(!matches("test", &tiles, "Backlog"));
        // Case and accents are ignored.
        assert!(matches("cafe", &task("Café crème", "", 0), "Backlog"));
    }

    #[test]
    fn fzf_syntax_works() {
        let readme = task("Write the README", "Cover install and usage", 0);
        assert!(!matches("!usage", &readme, "Backlog"));
        assert!(matches("!deploy", &readme, "Backlog"));
        assert!(matches("^write", &readme, "Backlog"));
        assert!(!matches("'wrt", &readme, "Backlog"));
    }

    #[test]
    fn filter_terms() {
        let old = task("Old #ui work", "", 10 * DAY);
        let fresh = task("Fresh", "Needs a #docs-pass", HOUR);
        assert!(matches("in:progress", &old, "In Progress"));
        assert!(matches("in:in-progress", &old, "In Progress"));
        assert!(!matches("in:done", &old, "In Progress"));
        assert!(matches("#ui", &old, "Backlog"));
        assert!(!matches("#u", &old, "Backlog"));
        assert!(matches("#DOCS-PASS", &fresh, "Backlog"));
        assert!(!matches("#docs", &fresh, "Backlog"));
        assert!(matches("updated:<7d", &fresh, "Backlog"));
        assert!(!matches("updated:<7d", &old, "Backlog"));
        assert!(matches("updated:>1w", &old, "Backlog"));
        assert!(matches("updated:today", &fresh, "Backlog"));
        assert!(matches("created:2d", &fresh, "Backlog"));
        assert!(!matches("created:2d", &old, "Backlog"));
        // Every term must match.
        assert!(!matches("#ui fresh", &old, "Backlog"));
        // Unknown values are just text.
        let filter = Filter::parse("updated:soon in:", NOW);
        let kinds: Vec<&Term> = filter.terms().map(|(_, term)| term).collect();
        assert!(matches!(kinds[..], [Term::Text(_), Term::Text(_)]));
    }

    #[test]
    fn highlights_count_graphemes() {
        let filter = Filter::parse("rdme", NOW);
        assert_eq!(filter.title_highlights("README"), [0, 3, 4, 5]);
        let filter = Filter::parse("crème", NOW);
        assert_eq!(filter.title_highlights("🦀 crème"), [2, 3, 4, 5, 6]);
        assert!(filter.title_highlights("nothing").is_empty());
        let filter = Filter::parse("usage", NOW);
        assert_eq!(filter.description_highlights("fix usage"), [4, 5, 6, 7, 8]);
    }
}
