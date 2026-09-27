//! Descriptions as lightweight Markdown: each line parsed by
//! [`crate::markdown`] and wrapped by words into styled lines, with list
//! continuations indented under their text.

use unicode_segmentation::UnicodeSegmentation;

use super::text;
use crate::markdown::{self, Block, Emphasis};

/// A role for a piece of description text; the drawer maps it to a style.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Tone {
    Text,
    Muted,
    Faint,
    Bold,
    Italic,
    Code,
    Heading,
}

/// One wrapped line of a description.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RichLine {
    pub spans: Vec<(String, Tone)>,
    /// The checklist item this line belongs to, if any.
    pub item: Option<usize>,
}

impl RichLine {
    #[cfg(test)]
    pub fn text(&self) -> String {
        self.spans.iter().map(|(piece, _)| piece.as_str()).collect()
    }
}

/// A description wrapped to `width` cells.
pub(crate) fn lines(description: &str, width: usize) -> Vec<RichLine> {
    let mut lines = Vec::new();
    if width == 0 {
        return lines;
    }
    for block in markdown::blocks(description.trim_end()) {
        // Indentation is kept unless it would take most of the line.
        let indent = |indent: &str| {
            if indent.len() * 2 > width {
                String::new()
            } else {
                indent.to_owned()
            }
        };
        let (prefix, body, base, item): (Vec<(String, Tone)>, &str, Tone, Option<usize>) =
            match block {
                Block::Fence => continue,
                Block::Blank => {
                    lines.push(RichLine::default());
                    continue;
                }
                Block::Code { text } => {
                    for chunk in hard_wrap(text, width) {
                        lines.push(RichLine {
                            spans: vec![(chunk, Tone::Code)],
                            item: None,
                        });
                    }
                    continue;
                }
                Block::Heading { text, .. } => (Vec::new(), text, Tone::Heading, None),
                Block::Check {
                    indent: spaces,
                    checked,
                    text,
                    item,
                } => (
                    vec![(
                        format!("{}{} ", indent(spaces), markdown::check_box(checked)),
                        if checked { Tone::Faint } else { Tone::Muted },
                    )],
                    text,
                    if checked { Tone::Faint } else { Tone::Text },
                    Some(item),
                ),
                Block::Bullet {
                    indent: spaces,
                    text,
                } => (
                    vec![(format!("{}• ", indent(spaces)), Tone::Faint)],
                    text,
                    Tone::Text,
                    None,
                ),
                Block::Numbered {
                    indent: spaces,
                    number,
                    text,
                } => (
                    vec![(format!("{}{number} ", indent(spaces)), Tone::Faint)],
                    text,
                    Tone::Text,
                    None,
                ),
                Block::Quote { text } => (
                    vec![("│ ".to_owned(), Tone::Faint)],
                    text,
                    Tone::Italic,
                    None,
                ),
                Block::Text {
                    indent: spaces,
                    text,
                } => (vec![(indent(spaces), Tone::Text)], text, Tone::Text, None),
            };
        let pieces = markdown::inline(body)
            .into_iter()
            .map(|(piece, emphasis)| (piece, tone(base, emphasis)))
            .collect();
        for spans in wrap(prefix, pieces, width) {
            lines.push(RichLine { spans, item });
        }
    }
    lines
}

fn tone(base: Tone, emphasis: Emphasis) -> Tone {
    match (base, emphasis) {
        (Tone::Faint, _) => Tone::Faint,
        (_, Emphasis::Code) => Tone::Code,
        (Tone::Heading, _) => Tone::Heading,
        (_, Emphasis::Bold) => Tone::Bold,
        (_, Emphasis::Italic) => Tone::Italic,
        (base, Emphasis::Plain) => base,
    }
}

/// Cuts text into lines of at most `width` cells, keeping spaces, for
/// code.
fn hard_wrap(line: &str, width: usize) -> Vec<String> {
    let mut chunks = vec![String::new()];
    for grapheme in line.graphemes(true) {
        let current = chunks.last_mut().expect("never empty");
        if text::width(current) + text::width(grapheme) > width && !current.is_empty() {
            chunks.push(String::new());
        }
        chunks.last_mut().expect("never empty").push_str(grapheme);
    }
    chunks
}

/// Wraps styled pieces to `width` cells, breaking between words (and
/// inside words too long for a line). The first line starts with
/// `prefix`; the others are indented to line up after it.
fn wrap(
    prefix: Vec<(String, Tone)>,
    pieces: Vec<(String, Tone)>,
    width: usize,
) -> Vec<Vec<(String, Tone)>> {
    // Words, each made of one or more styled parts.
    let mut words: Vec<Vec<(String, Tone)>> = Vec::new();
    let mut word: Vec<(String, Tone)> = Vec::new();
    for (piece, tone) in pieces {
        for (index, part) in piece.split(' ').enumerate() {
            if index > 0 && !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            if !part.is_empty() {
                word.push((part.to_owned(), tone));
            }
        }
    }
    if !word.is_empty() {
        words.push(word);
    }

    let hanging: usize = prefix.iter().map(|(piece, _)| text::width(piece)).sum();
    let hanging = if hanging * 2 > width { 0 } else { hanging };
    let mut lines = Vec::new();
    let mut line = prefix;
    let mut used = hanging;
    let mut has_words = false;
    for word in words {
        let word_width: usize = word.iter().map(|(piece, _)| text::width(piece)).sum();
        if has_words && used + 1 + word_width > width {
            lines.push(std::mem::take(&mut line));
            line.push((" ".repeat(hanging), Tone::Text));
            used = hanging;
            has_words = false;
        }
        if has_words {
            // A space inside a code span keeps its background; one
            // between differently styled words doesn't.
            let tone = match line.last() {
                Some((_, last)) if *last == word[0].1 => *last,
                _ => Tone::Text,
            };
            push(&mut line, " ", tone);
            used += 1;
        }
        for (piece, tone) in word {
            for grapheme in piece.graphemes(true) {
                let grapheme_width = text::width(grapheme);
                if used + grapheme_width > width && used > hanging {
                    lines.push(std::mem::take(&mut line));
                    line.push((" ".repeat(hanging), Tone::Text));
                    used = hanging;
                }
                push(&mut line, grapheme, tone);
                used += grapheme_width;
            }
        }
        has_words = true;
    }
    lines.push(line);
    lines
}

/// Adds text to a line, joining it to the last piece when the tone is the
/// same.
fn push(line: &mut Vec<(String, Tone)>, text: &str, tone: Tone) {
    match line.last_mut() {
        Some((last, last_tone)) if *last_tone == tone => last.push_str(text),
        _ => line.push((text.to_owned(), tone)),
    }
}

/// A description's first line with content, as plain text, for the
/// preview on a card.
pub(crate) fn preview(description: &str) -> Option<String> {
    markdown::blocks(description)
        .iter()
        .map(markdown::plain)
        .map(|line| line.trim().to_owned())
        .find(|line| !line.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(description: &str, width: usize) -> Vec<String> {
        lines(description, width)
            .iter()
            .map(RichLine::text)
            .collect()
    }

    #[test]
    fn plain_text_keeps_indentation() {
        assert_eq!(
            texts("Intro\n\n    indented text here\nlast", 12),
            ["Intro", "", "    indented", "    text", "    here", "last"]
        );
    }

    #[test]
    fn lists_hang_under_their_text() {
        assert_eq!(
            texts("- one two three\n- [ ] check this box\n1. first step", 10),
            [
                "• one two",
                "  three",
                "☐ check",
                "  this box",
                "1. first",
                "   step"
            ]
        );
        let lines = lines("- [ ] a\n- [x] b", 20);
        assert_eq!(lines[0].item, Some(0));
        assert_eq!(lines[1].item, Some(1));
        assert!(lines[1].spans.iter().all(|(_, tone)| *tone == Tone::Faint));
    }

    #[test]
    fn markup_becomes_tones() {
        let lines = lines("# The **plan**\nuse `cargo test` *now*", 40);
        assert_eq!(lines[0].spans, [("The plan".to_owned(), Tone::Heading)]);
        assert_eq!(lines[1].text(), "use cargo test now");
        assert!(
            lines[1]
                .spans
                .contains(&("cargo test".to_owned(), Tone::Code))
        );
        assert!(lines[1].spans.contains(&("now".to_owned(), Tone::Italic)));
    }

    #[test]
    fn code_keeps_its_spaces_and_breaks_anywhere() {
        assert_eq!(texts("```\nfn  main()\n```", 6), ["fn  ma", "in()"]);
    }

    #[test]
    fn long_words_break_and_lines_never_overflow() {
        for line in texts(
            "- 日本語のタスク名と絵文字🦀🚀が混ざった supercalifragilistic",
            9,
        ) {
            assert!(text::width(&line) <= 9, "{line}");
        }
    }

    #[test]
    fn previews_skip_blank_lines_and_markup() {
        assert_eq!(preview("\n\n## **Goal**\nmore").as_deref(), Some("Goal"));
        assert_eq!(preview("```\n\n```").as_deref(), None);
        assert_eq!(preview("- [ ] first").as_deref(), Some("☐ first"));
    }
}
