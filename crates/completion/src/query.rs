use tree_sitter::Node;

use crate::catalog::{Identifier, column_suggestion};
use crate::{Cursor, Suggestion, Table};

const IDENTIFIER: &str = "identifier";
const REFERENCE: &str = "object_reference";
const RELATION: &str = "relation";
const FIELD: &str = "field";
const STATEMENT: &str = "statement";
const SUBQUERY: &str = "subquery";
const CTE: &str = "cte";
const SET_OPERATION: &str = "set_operation";
const ALIAS: &str = "alias";
const RELATION_CLAUSES: &[&str] = &["from", "join", "cross_join"];

struct Binding<'catalog, 'sql> {
    table: &'catalog Table,
    alias: Option<Identifier<'sql>>,
}

pub(crate) fn suggestions(
    target: Node<'_>,
    sql: &str,
    tables: &[Table],
    cursor: &Cursor,
) -> Vec<Suggestion> {
    if target.kind() != IDENTIFIER {
        return Vec::new();
    }
    let Some(parent) = target.parent() else {
        return Vec::new();
    };
    if parent.kind() == REFERENCE {
        if parent.named_child(parent.named_child_count() - 1) != Some(target)
            || parent.parent().is_none_or(|node| node.kind() != RELATION)
        {
            return Vec::new();
        }
        let mut qualifiers = identifiers(parent, sql);
        qualifiers.pop();
        return tables
            .iter()
            .filter_map(|table| table.suggestion(&qualifiers, cursor))
            .collect();
    }
    if parent.kind() != FIELD {
        return Vec::new();
    }
    let Some(scope) = scope(parent) else {
        return Vec::new();
    };
    let qualifiers = parent
        .named_child(0)
        .filter(|node| node.kind() == REFERENCE)
        .map_or_else(Vec::new, |reference| identifiers(reference, sql));
    let mut relations = Vec::new();
    collect_relations(scope, &mut relations);
    let bindings = relations
        .into_iter()
        .map(|relation| resolve(relation, sql, tables))
        .collect::<Vec<_>>();
    columns(&bindings, &qualifiers, cursor)
}

fn scope(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        match node.kind() {
            STATEMENT | SUBQUERY => return Some(node),
            SET_OPERATION => return None,
            _ => node = node.parent()?,
        }
    }
}

fn identifiers<'sql>(node: Node<'_>, sql: &'sql str) -> Vec<Identifier<'sql>> {
    node.named_children(&mut node.walk())
        .filter(|child| child.kind() == IDENTIFIER)
        .map(|child| Identifier::parse(&sql[child.byte_range()]))
        .collect()
}

fn collect_relations<'tree>(node: Node<'tree>, relations: &mut Vec<Node<'tree>>) {
    for child in node.named_children(&mut node.walk()) {
        if child.kind() == RELATION {
            relations.push(child);
        } else if RELATION_CLAUSES.contains(&child.kind()) {
            collect_relations(child, relations);
        }
    }
}

fn resolve<'catalog, 'sql>(
    relation: Node<'_>,
    sql: &'sql str,
    tables: &'catalog [Table],
) -> Option<Binding<'catalog, 'sql>> {
    let reference = relation
        .named_child(0)
        .filter(|node| node.kind() == REFERENCE)?;
    let names = identifiers(reference, sql);
    if names.len() == 1 && shadows_table(relation, &names[0], sql) {
        return None;
    }
    let mut candidates = tables.iter().filter(|table| table.matches(&names));
    let table = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    Some(Binding {
        table,
        alias: relation
            .child_by_field_name(ALIAS)
            .map(|alias| Identifier::parse(&sql[alias.byte_range()])),
    })
}

fn shadows_table(mut node: Node<'_>, name: &Identifier<'_>, sql: &str) -> bool {
    while let Some(parent) = node.parent() {
        for child in parent.named_children(&mut parent.walk()) {
            if child.kind() != CTE {
                continue;
            }
            if child.named_child(0).is_some_and(|identifier| {
                name.matches(&Identifier::parse(&sql[identifier.byte_range()]).text)
            }) {
                return true;
            }
        }
        node = parent;
    }
    false
}

fn columns(
    bindings: &[Option<Binding<'_, '_>>],
    qualifiers: &[Identifier<'_>],
    cursor: &Cursor,
) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();
    for binding in bindings.iter().flatten() {
        if !qualifiers.is_empty() && !binding.matches(qualifiers) {
            continue;
        }
        let qualifier = if qualifiers.is_empty() && bindings.len() > 1 {
            match &binding.alias {
                Some(alias) => vec![alias.text.as_ref()],
                None => binding.table.path(),
            }
        } else {
            Vec::new()
        };
        for column in &binding.table.columns {
            if cursor.matches(&column.name) {
                suggestions.push(column_suggestion(column, binding.table, &qualifier, cursor));
            }
        }
    }
    suggestions
}

impl Binding<'_, '_> {
    fn matches(&self, qualifiers: &[Identifier<'_>]) -> bool {
        match &self.alias {
            Some(alias) => qualifiers.len() == 1 && qualifiers[0].matches(&alias.text),
            None => self.table.matches(qualifiers),
        }
    }
}
