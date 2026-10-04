use std::borrow::Cow;

use crate::cursor::{closing_quote, identifier_character, unescape_identifier};
use crate::{Column, Cursor, Kind, Suggestion, Table};

pub(crate) struct Identifier<'sql> {
    pub(crate) text: Cow<'sql, str>,
    quoted: bool,
}

impl<'sql> Identifier<'sql> {
    pub(crate) fn parse(text: &'sql str) -> Self {
        let Some(quote) = text
            .chars()
            .next()
            .filter(|quote| matches!(quote, '"' | '`' | '['))
        else {
            return Self {
                text: Cow::Borrowed(text),
                quoted: false,
            };
        };
        let closing = closing_quote(quote);
        let text = &text[quote.len_utf8()..text.len() - closing.len_utf8()];
        Self {
            text: Cow::Owned(unescape_identifier(text, quote)),
            quoted: true,
        }
    }

    pub(crate) fn matches(&self, name: &str) -> bool {
        if self.quoted {
            self.text == name
        } else {
            self.text.eq_ignore_ascii_case(name)
        }
    }
}

impl Table {
    pub(crate) fn path(&self) -> Vec<&str> {
        self.catalog
            .as_deref()
            .into_iter()
            .chain(self.database_schema.as_deref())
            .chain(std::iter::once(self.name.as_str()))
            .collect()
    }

    pub(crate) fn matches(&self, names: &[Identifier<'_>]) -> bool {
        let path = self.path();
        names.len() <= path.len()
            && names
                .iter()
                .zip(&path[path.len() - names.len()..])
                .all(|(name, part)| name.matches(part))
    }

    pub(crate) fn suggestion(
        &self,
        names: &[Identifier<'_>],
        cursor: &Cursor,
    ) -> Option<Suggestion> {
        let path = self.path();
        let remaining = if names.is_empty() {
            if !cursor.matches(&self.name) && !cursor.matches(path[0]) {
                return None;
            }
            path.as_slice()
        } else {
            let start = (0..path.len()).find(|start| {
                let suffix = &path[*start..];
                names.len() < suffix.len()
                    && names
                        .iter()
                        .zip(suffix)
                        .all(|(name, part)| name.matches(part))
                    && cursor.matches(suffix[names.len()])
            })?;
            &path[start + names.len()..]
        };
        Some(Suggestion {
            label: path.join("."),
            detail: "Table".to_owned(),
            insertion: quoted_path(remaining, cursor.quote),
            kind: Kind::Table,
            replacement: cursor.replacement.clone(),
        })
    }
}

impl Cursor {
    pub(crate) fn matches(&self, name: &str) -> bool {
        match self.quote {
            Some(_) => name.starts_with(&self.prefix),
            None => name.to_lowercase().starts_with(&self.prefix),
        }
    }
}

pub(crate) fn quoted_path(path: &[&str], quote: Option<char>) -> String {
    path.iter()
        .map(|part| {
            if quote.is_none() && plain_identifier(part) {
                return (*part).to_owned();
            }
            let quote = quote.unwrap_or('"');
            let closing = closing_quote(quote);
            let escaped = part.replace(closing, &closing.to_string().repeat(2));
            format!("{quote}{escaped}{closing}")
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn plain_identifier(name: &str) -> bool {
    let language: tree_sitter::Language = tree_sitter_sequel::LANGUAGE.into();
    name.chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && name
            .chars()
            .all(|character| character.is_ascii() && identifier_character(character))
        && language.id_for_node_kind(&format!("keyword_{}", name.to_ascii_lowercase()), true) == 0
}

pub(crate) fn column_suggestion(
    column: &Column,
    table: &Table,
    qualifier: &[&str],
    cursor: &Cursor,
) -> Suggestion {
    let insertion = if qualifier.is_empty() {
        quoted_path(&[column.name.as_str()], cursor.quote)
    } else {
        format!(
            "{}.{}",
            quoted_path(qualifier, None),
            quoted_path(&[column.name.as_str()], cursor.quote)
        )
    };
    Suggestion {
        label: if qualifier.is_empty() {
            column.name.clone()
        } else {
            insertion.clone()
        },
        detail: format!("{} · {}", column.data_type, table.path().join(".")),
        insertion,
        kind: Kind::Column,
        replacement: cursor.replacement.clone(),
    }
}
