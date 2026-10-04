use std::ops::Range;

#[derive(Debug)]
pub struct Column {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug)]
pub struct Table {
    pub catalog: Option<String>,
    pub database_schema: Option<String>,
    pub name: String,
    pub columns: Vec<Column>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Table,
    Column,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub label: String,
    pub detail: String,
    pub insertion: String,
    pub kind: Kind,
    pub replacement: Range<usize>,
}

mod catalog;
mod cursor;
mod query;

use cursor::Cursor;
use tree_sitter::{InputEdit, Parser, Point, Tree};

const COMPLETION_IDENTIFIER: &str = "__sql_bomb_completion__";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not load the SQL grammar: {0}")]
    Grammar(#[from] tree_sitter::LanguageError),
    #[error("Cursor {0} is not a UTF-8 boundary in the SQL text; use the editor's byte offset")]
    Cursor(usize),
    #[error("SQL parsing was interrupted; edit the query and try completion again")]
    Interrupted,
}

pub struct Engine {
    parser: Parser,
    text: String,
    tree: Option<Tree>,
}

impl Engine {
    pub fn new() -> Result<Self, Error> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_sequel::LANGUAGE.into())?;
        Ok(Self {
            parser,
            text: String::new(),
            tree: None,
        })
    }

    /// Cursor and replacement ranges are UTF-8 byte offsets into `sql`.
    pub fn complete(
        &mut self,
        sql: &str,
        cursor: usize,
        tables: &[Table],
    ) -> Result<Vec<Suggestion>, Error> {
        if !sql.is_char_boundary(cursor) {
            return Err(Error::Cursor(cursor));
        }
        let Some(cursor) = cursor::at(sql, cursor) else {
            return Ok(Vec::new());
        };
        let mut text = String::with_capacity(sql.len() + COMPLETION_IDENTIFIER.len());
        text.push_str(&sql[..cursor.replacement.start]);
        text.push_str(COMPLETION_IDENTIFIER);
        text.push_str(&sql[cursor.replacement.end..]);
        self.parse(text)?;
        let tree = self
            .tree
            .as_ref()
            .expect("parse stores a syntax tree on success");
        let target = tree.root_node().descendant_for_byte_range(
            cursor.replacement.start,
            cursor.replacement.start + COMPLETION_IDENTIFIER.len(),
        );
        let mut suggestions = target.map_or_else(Vec::new, |target| {
            query::suggestions(target, &self.text, tables, &cursor)
        });
        suggestions.sort_unstable_by(|left, right| left.label.cmp(&right.label));
        suggestions.dedup();
        Ok(suggestions)
    }

    fn parse(&mut self, text: String) -> Result<(), Error> {
        if text == self.text && self.tree.is_some() {
            return Ok(());
        }
        let mut previous = self.tree.take();
        if let Some(tree) = &mut previous {
            tree.edit(&difference(&self.text, &text));
        }
        self.tree = Some(
            self.parser
                .parse(&text, previous.as_ref())
                .ok_or(Error::Interrupted)?,
        );
        self.text = text;
        Ok(())
    }
}

fn difference(previous: &str, text: &str) -> InputEdit {
    let start = previous
        .chars()
        .zip(text.chars())
        .take_while(|(old, new)| old == new)
        .map(|(character, _)| character.len_utf8())
        .sum();
    let suffix: usize = previous[start..]
        .chars()
        .rev()
        .zip(text[start..].chars().rev())
        .take_while(|(old, new)| old == new)
        .map(|(character, _)| character.len_utf8())
        .sum();
    let old_end = previous.len() - suffix;
    let new_end = text.len() - suffix;
    InputEdit {
        start_byte: start,
        old_end_byte: old_end,
        new_end_byte: new_end,
        start_position: position(&text[..start]),
        old_end_position: position(&previous[..old_end]),
        new_end_position: position(&text[..new_end]),
    }
}

fn position(prefix: &str) -> Point {
    Point {
        row: prefix.bytes().filter(|byte| *byte == b'\n').count(),
        column: prefix
            .rfind('\n')
            .map_or(prefix.len(), |offset| prefix.len() - offset - 1),
    }
}
