use std::ops::Range;

const LINE_COMMENT: &str = "--";
const BLOCK_COMMENT_START: &str = "/*";
const BLOCK_COMMENT_END: &str = "*/";

struct Comment {
    end: usize,
    closed: bool,
}

pub(crate) struct Cursor {
    pub(crate) replacement: Range<usize>,
    pub(crate) prefix: String,
    pub(crate) quote: Option<char>,
}

pub(crate) fn at(sql: &str, position: usize) -> Option<Cursor> {
    let mut offset = 0;
    while offset < sql.len() && offset <= position {
        let remaining = &sql[offset..];
        let character = remaining.chars().next()?;
        if let Some(comment) = comment(remaining) {
            let end = offset + comment.end;
            if position < end || position == end && !comment.closed {
                return None;
            }
            offset = end;
            continue;
        }
        if let Some(delimiter) = dollar_delimiter(remaining) {
            let end = remaining[delimiter.len()..]
                .find(delimiter)
                .map_or(sql.len(), |end| offset + delimiter.len() * 2 + end);
            if position <= end {
                return None;
            }
            offset = end;
            continue;
        }
        if matches!(character, '\'' | '"' | '`' | '[') {
            let closing = quote_ending(remaining, character);
            let end = closing.end + offset;
            if (offset..=end).contains(&position) {
                return quoted(
                    sql,
                    offset..end,
                    position,
                    character,
                    offset + closing.start,
                );
            }
            offset = end;
            continue;
        }
        if identifier_character(character) {
            let length = remaining
                .find(|character| !identifier_character(character))
                .unwrap_or(remaining.len());
            let end = offset + length;
            if (offset..=end).contains(&position) {
                return (!character.is_numeric()).then_some(Cursor {
                    replacement: offset..end,
                    prefix: sql[offset..position].to_lowercase(),
                    quote: None,
                });
            }
            offset = end;
            continue;
        }
        offset += character.len_utf8();
    }
    Some(Cursor {
        replacement: position..position,
        prefix: String::new(),
        quote: None,
    })
}

fn quoted(
    sql: &str,
    range: Range<usize>,
    position: usize,
    quote: char,
    content_end: usize,
) -> Option<Cursor> {
    if quote == '\'' || position == range.start {
        return None;
    }
    let prefix = unescape_identifier(
        &sql[range.start + quote.len_utf8()..position.min(content_end)],
        quote,
    );
    Some(Cursor {
        replacement: range,
        prefix,
        quote: Some(quote),
    })
}

pub(crate) fn unescape_identifier(text: &str, quote: char) -> String {
    let closing = closing_quote(quote).to_string();
    text.replace(&closing.repeat(2), &closing)
}

pub(crate) fn closing_quote(quote: char) -> char {
    if quote == '[' { ']' } else { quote }
}

pub(crate) fn identifier_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '$')
}

fn quote_ending(sql: &str, quote: char) -> Range<usize> {
    let closing = closing_quote(quote);
    let mut characters = sql.char_indices().skip(1).peekable();
    while let Some((offset, character)) = characters.next() {
        if character == '\\' && quote == '\'' {
            characters.next();
            continue;
        }
        if character != closing {
            continue;
        }
        if characters.peek().is_some_and(|(_, next)| *next == closing) {
            characters.next();
            continue;
        }
        return offset..offset + character.len_utf8();
    }
    sql.len()..sql.len()
}

fn comment(sql: &str) -> Option<Comment> {
    if sql.starts_with(LINE_COMMENT) {
        return Some(Comment {
            end: sql.find('\n').unwrap_or(sql.len()),
            closed: false,
        });
    }
    if !sql.starts_with(BLOCK_COMMENT_START) {
        return None;
    }
    let mut depth = 1;
    let mut offset = BLOCK_COMMENT_START.len();
    while offset < sql.len() {
        let remaining = &sql[offset..];
        if remaining.starts_with(BLOCK_COMMENT_START) {
            depth += 1;
            offset += BLOCK_COMMENT_START.len();
        } else if remaining.starts_with(BLOCK_COMMENT_END) {
            depth -= 1;
            offset += BLOCK_COMMENT_END.len();
            if depth == 0 {
                return Some(Comment {
                    end: offset,
                    closed: true,
                });
            }
        } else {
            offset += 1;
            while !sql.is_char_boundary(offset) {
                offset += 1;
            }
        }
    }
    Some(Comment {
        end: sql.len(),
        closed: false,
    })
}

fn dollar_delimiter(sql: &str) -> Option<&str> {
    let remaining = sql.strip_prefix('$')?;
    let end = remaining.find('$')?;
    remaining[..end]
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
        .then_some(&sql[..end + 2])
}
