//! Measuring, cutting and wrapping text by terminal cells, and formatting
//! times.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// The width of `text` in terminal cells.
pub(crate) fn width(text: &str) -> usize {
    text.width()
}

/// Cuts `text` to at most `max_width` terminal cells, ending with "…" when
/// anything was removed. Text that fits is returned unchanged. Works on
/// grapheme clusters, so combining marks stay with their base character
/// and wide characters are never split.
pub(crate) fn truncate_text(text: &str, max_width: usize) -> String {
    if text.width() <= max_width {
        return text.to_owned();
    }
    if max_width == 0 {
        return String::new();
    }
    let mut result = take_width(text, max_width - 1).to_owned();
    result.push('…');
    result
}

/// The longest prefix of `text` that is at most `max_width` cells wide.
fn take_width(text: &str, max_width: usize) -> &str {
    let mut used = 0;
    let mut end = 0;
    for (index, grapheme) in text.grapheme_indices(true) {
        let width = grapheme.width();
        if used + width > max_width {
            break;
        }
        used += width;
        end = index + grapheme.len();
    }
    &text[..end]
}

/// The part of an input's value to show in a field `width` cells wide,
/// scrolled so the cursor (a byte offset) stays visible, and the cursor's
/// cell offset within it.
pub(crate) fn input_view(value: &str, cursor: usize, width: usize) -> (String, usize) {
    let mut cursor = cursor.min(value.len());
    while !value.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let before = &value[..cursor];
    // Keep a cell free for the cursor itself.
    let room = width.saturating_sub(1);
    let mut start = cursor;
    let mut used = 0;
    for (index, grapheme) in before.grapheme_indices(true).rev() {
        let grapheme_width = grapheme.width();
        if used + grapheme_width > room {
            break;
        }
        used += grapheme_width;
        start = index;
    }
    if used == before.width() {
        start = 0;
    }
    (take_width(&value[start..], width).to_owned(), used)
}

/// Wraps `text` to lines of at most `max_width` cells, breaking at spaces
/// where possible and inside words that are too long for a line. At most
/// `max_lines` lines are returned; if the text didn't fit, the last one
/// ends with "…". Whitespace runs are collapsed, so this is for titles
/// rather than descriptions whose layout matters.
pub(crate) fn wrap(text: &str, max_width: usize, max_lines: usize) -> Vec<String> {
    if max_width == 0 || max_lines == 0 {
        return Vec::new();
    }
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word = word;
        loop {
            let separator = usize::from(!line.is_empty());
            if line.width() + separator + word.width() <= max_width {
                if separator == 1 {
                    line.push(' ');
                }
                line.push_str(word);
                break;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                continue;
            }
            // The word alone is too long for a line: break it.
            let mut head = take_width(word, max_width);
            if head.is_empty() {
                // A character wider than the line; skip it.
                head = word.graphemes(true).next().unwrap_or(word);
            } else {
                lines.push(head.to_owned());
            }
            word = &word[head.len()..];
            if word.is_empty() {
                break;
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.last_mut().expect("max_lines is at least one");
        let mut cut = take_width(last, max_width.saturating_sub(1)).to_owned();
        cut.push('…');
        *last = cut;
    }
    lines
}

/// How long ago `timestamp` was, as "just now", "5m ago", "3h ago" or
/// "2d ago", or as a date ("Aug 20", or "Aug 29, 2025" in another year)
/// after a week. Both times are Unix milliseconds; dates are in UTC.
pub(crate) fn relative_time(timestamp: i64, now: i64) -> String {
    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let elapsed = now.saturating_sub(timestamp).max(0);
    match elapsed {
        0..MINUTE => "just now".to_owned(),
        MINUTE..HOUR => format!("{}m ago", elapsed / MINUTE),
        HOUR..DAY => format!("{}h ago", elapsed / HOUR),
        _ if elapsed < 7 * DAY => format!("{}d ago", elapsed / DAY),
        _ => short_date(timestamp, now),
    }
}

/// "Sep 3", or "Sep 3, 2025" when the year differs from `now`'s.
pub(crate) fn short_date(timestamp: i64, now: i64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (year, month, day) = civil_date(timestamp);
    let month = MONTHS[(month - 1) as usize];
    if year == civil_date(now).0 {
        format!("{month} {day}")
    } else {
        format!("{month} {day}, {year}")
    }
}

/// The UTC (year, month, day) of a Unix millisecond timestamp, using
/// Howard Hinnant's `civil_from_days`.
fn civil_date(timestamp: i64) -> (i64, u32, u32) {
    let days = timestamp.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_respects_the_requested_width() {
        assert_eq!(truncate_text("abcdef", 4), "abc…");
        assert_eq!(truncate_text("abc", 1), "…");
        assert_eq!(truncate_text("abc", 0), "");
        assert_eq!(truncate_text("", 0), "");
    }

    #[test]
    fn truncation_keeps_text_that_fits_exactly() {
        assert_eq!(truncate_text("abcd", 4), "abcd");
        assert_eq!(truncate_text("abcd", 5), "abcd");
        assert_eq!(truncate_text("日本", 4), "日本");
    }

    #[test]
    fn truncation_measures_wide_characters() {
        // Each CJK character is two cells wide.
        assert_eq!(truncate_text("日本語", 5), "日本…");
        assert_eq!(truncate_text("日本語", 4), "日…");
        // A two-cell character never overflows a one-cell budget.
        assert_eq!(truncate_text("日本", 1), "…");
        assert_eq!(truncate_text("a🦀b", 3), "a…");
    }

    #[test]
    fn truncation_keeps_combining_marks_with_their_base() {
        // "e" + combining acute accent is one cell wide.
        let text = "e\u{301}e\u{301}e\u{301}";
        assert_eq!(truncate_text(text, 3), text);
        assert_eq!(truncate_text(text, 2), "e\u{301}…");
    }

    #[test]
    fn truncation_handles_long_text_quickly() {
        let text = "x".repeat(100_000);
        let cut = truncate_text(&text, 50_000);
        assert_eq!(cut.width(), 50_000);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn input_view_keeps_the_cursor_visible() {
        assert_eq!(input_view("hello", 5, 10), ("hello".to_owned(), 5));
        assert_eq!(input_view("hello", 0, 10), ("hello".to_owned(), 0));
        assert_eq!(input_view("hello world", 11, 6), ("world".to_owned(), 5));
        assert_eq!(input_view("hello world", 2, 6), ("hello ".to_owned(), 2));
        assert_eq!(input_view("日本語", 9, 5), ("本語".to_owned(), 4));
    }

    #[test]
    fn wrap_breaks_at_spaces() {
        assert_eq!(wrap("one two three", 7, 3), ["one two", "three"]);
        assert_eq!(wrap("one two three four", 7, 2), ["one two", "three…"]);
        assert_eq!(wrap("  spaced   out  ", 20, 2), ["spaced out"]);
        assert!(wrap("", 10, 2).is_empty());
        assert!(wrap("text", 0, 2).is_empty());
    }

    #[test]
    fn wrap_breaks_long_words() {
        assert_eq!(wrap("abcdefgh", 3, 3), ["abc", "def", "gh"]);
        assert_eq!(wrap("aaa bbb ccc", 3, 2), ["aaa", "bb…"]);
        assert_eq!(wrap("日本語です", 4, 3), ["日本", "語で", "す"]);
        // Never wider than the line, even with wide characters.
        for line in wrap("日本語のタスク名と絵文字 🦀🚀 が混ざった", 5, 10) {
            assert!(line.width() <= 5, "{line}");
        }
    }

    #[test]
    fn relative_times() {
        let now = 1_788_000_000_000; // 2026-08-29 10:40 UTC
        assert_eq!(relative_time(now - 30_000, now), "just now");
        assert_eq!(relative_time(now - 5 * 60_000, now), "5m ago");
        assert_eq!(relative_time(now - 3 * 3_600_000, now), "3h ago");
        assert_eq!(relative_time(now - 3 * 86_400_000, now), "3d ago");
        assert_eq!(relative_time(now - 9 * 86_400_000, now), "Aug 20");
        assert_eq!(relative_time(now - 365 * 86_400_000, now), "Aug 29, 2025");
        // A timestamp in the future reads as now.
        assert_eq!(relative_time(now + 60_000, now), "just now");
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(1_788_000_000_000), (2026, 8, 29));
        assert_eq!(civil_date(951_782_400_000), (2000, 2, 29));
        assert_eq!(civil_date(-86_400_000), (1969, 12, 31));
    }
}
