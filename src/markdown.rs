//! The small part of Markdown that task descriptions use: headings,
//! lists, `- [ ]` checklists, quotes, fenced code, and inline bold,
//! italic and `code`. Parsing is line by line and never fails: anything
//! it doesn't recognise is plain text.

/// How a piece of inline text is emphasised.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Emphasis {
    Plain,
    Bold,
    Italic,
    Code,
}

/// One line of a description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Block<'a> {
    Blank,
    /// `# Title` to `###### Title`.
    Heading {
        level: usize,
        text: &'a str,
    },
    /// `- [ ] item` or `- [x] item`; `item` counts the checklist items
    /// before this one.
    Check {
        indent: &'a str,
        checked: bool,
        text: &'a str,
        item: usize,
    },
    /// `- item`, `* item` or `+ item`.
    Bullet {
        indent: &'a str,
        text: &'a str,
    },
    /// `1. item` or `1) item`.
    Numbered {
        indent: &'a str,
        number: &'a str,
        text: &'a str,
    },
    /// `> quote`.
    Quote {
        text: &'a str,
    },
    /// A line inside a fenced code block, as written.
    Code {
        text: &'a str,
    },
    /// A ```` ``` ```` fence line.
    Fence,
    Text {
        indent: &'a str,
        text: &'a str,
    },
}

/// Parses a description into blocks, one per line.
pub fn blocks(text: &str) -> Vec<Block<'_>> {
    let mut in_code = false;
    let mut item = 0;
    text.lines()
        .map(|line| {
            if line.trim_start().starts_with("```") {
                in_code = !in_code;
                return Block::Fence;
            }
            if in_code {
                return Block::Code { text: line };
            }
            let block = parse_line(line, item);
            if matches!(block, Block::Check { .. }) {
                item += 1;
            }
            block
        })
        .collect()
}

fn parse_line(line: &str, item: usize) -> Block<'_> {
    let content = line.trim_start();
    let indent = &line[..line.len() - content.len()];
    if content.is_empty() {
        return Block::Blank;
    }
    let hashes = content
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if (1..=6).contains(&hashes)
        && let Some(text) = content[hashes..].strip_prefix(' ')
    {
        return Block::Heading {
            level: hashes,
            text: text.trim(),
        };
    }
    if let Some(text) = content.strip_prefix('>') {
        return Block::Quote {
            text: text.trim_start(),
        };
    }
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = content.strip_prefix(marker) {
            for (box_text, checked) in [("[ ]", false), ("[x]", true), ("[X]", true)] {
                if let Some(text) = rest.strip_prefix(box_text)
                    && (text.is_empty() || text.starts_with(' '))
                {
                    return Block::Check {
                        indent,
                        checked,
                        text: text.trim_start(),
                        item,
                    };
                }
            }
            return Block::Bullet { indent, text: rest };
        }
    }
    let digits = content
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if (1..=9).contains(&digits)
        && let Some(text) = content[digits..]
            .strip_prefix(". ")
            .or_else(|| content[digits..].strip_prefix(") "))
    {
        return Block::Numbered {
            indent,
            number: &content[..digits + 1],
            text,
        };
    }
    Block::Text {
        indent,
        text: content,
    }
}

/// Splits inline text into emphasised pieces: `**bold**` or `__bold__`,
/// `*italic*` or `_italic_`, and `` `code` ``. Unclosed markers are kept
/// as text, and nothing is interpreted inside code.
pub fn inline(text: &str) -> Vec<(String, Emphasis)> {
    let mut pieces: Vec<(String, Emphasis)> = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    while let Some(character) = rest.chars().next() {
        let found = match character {
            '`' => span(rest, "`", Emphasis::Code),
            '*' => span(rest, "**", Emphasis::Bold).or_else(|| span(rest, "*", Emphasis::Italic)),
            // Underscores inside words (snake_case) are not emphasis.
            '_' if !plain.ends_with(char::is_alphanumeric) => {
                span(rest, "__", Emphasis::Bold).or_else(|| span(rest, "_", Emphasis::Italic))
            }
            _ => None,
        };
        match found {
            Some((inner, emphasis, used)) => {
                if !plain.is_empty() {
                    pieces.push((std::mem::take(&mut plain), Emphasis::Plain));
                }
                pieces.push((inner.to_owned(), emphasis));
                rest = &rest[used..];
            }
            None => {
                plain.push(character);
                rest = &rest[character.len_utf8()..];
            }
        }
    }
    if !plain.is_empty() {
        pieces.push((plain, Emphasis::Plain));
    }
    pieces
}

/// A span opened by `marker` at the start of `text`: its inner text, and
/// how many bytes it used. The inner text can't be empty or start or end
/// with a space (except in code).
fn span<'a>(text: &'a str, marker: &str, emphasis: Emphasis) -> Option<(&'a str, Emphasis, usize)> {
    let after = text.strip_prefix(marker)?;
    let end = after.find(marker)?;
    let inner = &after[..end];
    let code = emphasis == Emphasis::Code;
    if inner.is_empty() || (!code && (inner.starts_with(' ') || inner.ends_with(' '))) {
        return None;
    }
    Some((inner, emphasis, marker.len() * 2 + end))
}

/// A line as plain text, without its Markdown: for the one-line preview
/// on a card.
pub fn plain(block: &Block<'_>) -> String {
    let text = |text: &str| {
        inline(text)
            .into_iter()
            .map(|(piece, _)| piece)
            .collect::<String>()
    };
    match block {
        Block::Blank | Block::Fence => String::new(),
        Block::Heading { text: body, .. }
        | Block::Quote { text: body }
        | Block::Text { text: body, .. } => text(body),
        Block::Code { text } => text.trim().to_owned(),
        Block::Check {
            checked,
            text: body,
            ..
        } => {
            format!("{} {}", check_box(*checked), text(body))
        }
        Block::Bullet { text: body, .. } => format!("• {}", text(body)),
        Block::Numbered {
            number, text: body, ..
        } => format!("{number} {}", text(body)),
    }
}

pub fn check_box(checked: bool) -> &'static str {
    if checked { "☑" } else { "☐" }
}

/// How many checklist items are checked, out of how many; `None` when
/// there are none.
pub fn progress(text: &str) -> Option<(usize, usize)> {
    let checks: Vec<bool> = blocks(text)
        .iter()
        .filter_map(|block| match block {
            Block::Check { checked, .. } => Some(*checked),
            _ => None,
        })
        .collect();
    (!checks.is_empty()).then(|| {
        (
            checks.iter().filter(|checked| **checked).count(),
            checks.len(),
        )
    })
}

/// The description with checklist item `item` ticked or unticked, and
/// the item's text; `None` if there is no such item.
pub fn toggle(text: &str, item: usize) -> Option<(String, bool, String)> {
    let blocks = blocks(text);
    let (index, checked, label) =
        blocks
            .iter()
            .enumerate()
            .find_map(|(index, block)| match block {
                Block::Check {
                    item: this,
                    checked,
                    text,
                    ..
                } if *this == item => Some((index, *checked, text.to_string())),
                _ => None,
            })?;
    let lines: Vec<String> = text
        .lines()
        .enumerate()
        .map(|(line_index, line)| {
            if line_index != index {
                return line.to_owned();
            }
            let (from, to) = if checked {
                (["[x]", "[X]"].as_slice(), "[ ]")
            } else {
                (["[ ]"].as_slice(), "[x]")
            };
            let at = from
                .iter()
                .filter_map(|marker| line.find(marker))
                .min()
                .expect("a checklist line has its box");
            format!("{}{to}{}", &line[..at], &line[at + 3..])
        })
        .collect();
    let mut toggled = lines.join("\n");
    if text.ends_with('\n') {
        toggled.push('\n');
    }
    Some((toggled, !checked, label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_recognised() {
        let text = "# Plan\n\n- [ ] draft\n  - [x] outline\n* point\n2. second\n> note\n```\n# not a heading\n```\nplain";
        let blocks = blocks(text);
        assert_eq!(
            blocks[0],
            Block::Heading {
                level: 1,
                text: "Plan"
            }
        );
        assert_eq!(blocks[1], Block::Blank);
        assert_eq!(
            blocks[2],
            Block::Check {
                indent: "",
                checked: false,
                text: "draft",
                item: 0
            }
        );
        assert_eq!(
            blocks[3],
            Block::Check {
                indent: "  ",
                checked: true,
                text: "outline",
                item: 1
            }
        );
        assert_eq!(
            blocks[4],
            Block::Bullet {
                indent: "",
                text: "point"
            }
        );
        assert_eq!(
            blocks[5],
            Block::Numbered {
                indent: "",
                number: "2.",
                text: "second"
            }
        );
        assert_eq!(blocks[6], Block::Quote { text: "note" });
        assert_eq!(blocks[7], Block::Fence);
        assert_eq!(
            blocks[8],
            Block::Code {
                text: "# not a heading"
            }
        );
        assert_eq!(
            blocks[10],
            Block::Text {
                indent: "",
                text: "plain"
            }
        );
        // Not quite Markdown: plain text.
        assert!(matches!(parse_line("#hashtag", 0), Block::Text { .. }));
        assert!(matches!(parse_line("-dash", 0), Block::Text { .. }));
    }

    #[test]
    fn inline_emphasis() {
        use Emphasis::*;
        let pieces = |text| inline(text);
        assert_eq!(
            pieces("a **b** *c* `d*e*`"),
            [
                ("a ".to_owned(), Plain),
                ("b".to_owned(), Bold),
                (" ".to_owned(), Plain),
                ("c".to_owned(), Italic),
                (" ".to_owned(), Plain),
                ("d*e*".to_owned(), Code),
            ]
        );
        assert_eq!(
            pieces("snake_case_name"),
            [("snake_case_name".to_owned(), Plain)]
        );
        assert_eq!(pieces("2 * 3 * 4"), [("2 * 3 * 4".to_owned(), Plain)]);
        assert_eq!(pieces("**open"), [("**open".to_owned(), Plain)]);
        assert_eq!(pieces("__b__ _i_")[0], ("b".to_owned(), Bold));
    }

    #[test]
    fn checklists_count_and_toggle() {
        let text = "- [ ] one\n- [x] two\n```\n- [ ] in code\n```\n- [X] three\n";
        assert_eq!(progress(text), Some((2, 3)));
        assert_eq!(progress("no list"), None);
        let (toggled, checked, label) = toggle(text, 0).unwrap();
        assert!(checked);
        assert_eq!(label, "one");
        assert_eq!(progress(&toggled), Some((3, 3)));
        assert!(toggled.ends_with('\n'));
        let (toggled, checked, _) = toggle(&toggled, 2).unwrap();
        assert!(!checked);
        assert!(toggled.contains("- [ ] three"));
        assert!(toggled.contains("- [ ] in code"));
        assert_eq!(toggle(text, 3), None);
    }

    #[test]
    fn plain_previews() {
        let preview = |line: &str| plain(&blocks(line)[0]);
        assert_eq!(preview("# **Big** plan"), "Big plan");
        assert_eq!(preview("- [x] done"), "☑ done");
        assert_eq!(preview("- item"), "• item");
        assert_eq!(preview("> said"), "said");
    }
}
