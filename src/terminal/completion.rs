use super::Terminal;
use fusor::{OwnerHandle, Signal, signal};
use futures_util::future::{AbortHandle, Abortable};
use hypercmd::{Error, ErrorKind, Event, EventPayload, Input, Key, Node, Services};
use sql_bomb::QueryClient;
use sql_bomb_completion::{Column, Engine, Suggestion, Table};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const MAX_SUGGESTIONS: usize = 20;

enum Metadata {
    Unrequested,
    Loading(AbortHandle),
    Ready(Vec<Table>),
    Unavailable,
}

pub(super) struct Completion {
    engine: RefCell<Engine>,
    metadata: RefCell<Metadata>,
    editor: RefCell<Option<Node>>,
    pub(super) requested: Cell<bool>,
    pub(super) suggestions: Signal<Vec<Suggestion>>,
    pub(super) selected: Signal<usize>,
    pub(super) status: Signal<String>,
}

impl Completion {
    pub(super) fn new() -> Result<Self, Error> {
        Ok(Self {
            engine: RefCell::new(
                Engine::new()
                    .map_err(|error| Error::new(ErrorKind::Construction, error.to_string()))?,
            ),
            metadata: RefCell::new(Metadata::Unrequested),
            editor: RefCell::new(None),
            requested: Cell::new(false),
            suggestions: signal(Vec::new()),
            selected: signal(0),
            status: signal(String::new()),
        })
    }

    pub(super) fn dismiss(&self) {
        self.requested.set(false);
        self.suggestions.set(Vec::new());
        self.selected.set(0);
    }

    pub(super) fn reset(&self) {
        if let Metadata::Loading(abort) = self.metadata.replace(Metadata::Unrequested) {
            abort.abort();
        }
        self.status.set(String::new());
        self.dismiss();
    }

    fn load(
        self: &Rc<Self>,
        client: Rc<dyn QueryClient>,
        services: &Services,
        owner: &OwnerHandle,
    ) -> Result<(), Error> {
        if !matches!(*self.metadata.borrow(), Metadata::Unrequested) {
            return Ok(());
        }
        let completion = self.clone();
        let (abort, registration) = AbortHandle::new_pair();
        services.spawn(owner, async move {
            let discovery = async {
                match client.tables() {
                    Some(tables) => Some(tables.await),
                    None => None,
                }
            };
            // Switching connections cancels discovery before it can publish stale metadata.
            let Ok(result) = Abortable::new(discovery, registration).await else {
                return;
            };
            completion.received(result);
        })?;
        self.metadata.replace(Metadata::Loading(abort));
        self.status.set("Loading schema…".into());
        Ok(())
    }

    fn received(&self, result: Option<Result<Vec<sql_bomb::Table>, sql_bomb::Error>>) {
        let (metadata, status) = match result {
            Some(Ok(tables)) => {
                let status = format!("{} tables available", tables.len());
                (
                    Metadata::Ready(tables.into_iter().map(table).collect()),
                    status,
                )
            }
            Some(Err(error)) => (
                Metadata::Unavailable,
                format!("Suggestions unavailable: {error}"),
            ),
            None => (
                Metadata::Unavailable,
                "This client does not provide schema suggestions".into(),
            ),
        };
        self.metadata.replace(metadata);
        self.status.set(status);
        if !self.requested.get() {
            return;
        }
        if let Some(editor) = self.editor.borrow().as_ref() {
            self.refresh(editor);
        }
    }

    pub(super) fn refresh(&self, editor: &Node) {
        let cursor = editor.editor();
        self.dismiss();
        if cursor.anchor.is_some_and(|anchor| anchor != cursor.cursor) {
            return;
        }
        self.requested.set(true);
        let metadata = self.metadata.borrow();
        let Metadata::Ready(tables) = &*metadata else {
            return;
        };
        let suggestions = self
            .engine
            .borrow_mut()
            .complete(&editor.value(), cursor.cursor, tables);
        match suggestions {
            Ok(mut suggestions) => {
                suggestions.truncate(MAX_SUGGESTIONS);
                self.selected.set(0);
                self.suggestions.set(suggestions);
                if let Some(list) = editor.component_root().find("query-suggestions") {
                    list.scroll_to(0, 0);
                }
            }
            Err(error) => self.status.set(format!("Suggestions unavailable: {error}")),
        }
    }

    pub(super) fn accept(&self, suggestion: &Suggestion) -> Result<(), Error> {
        if let Some(editor) = self.editor.borrow().as_ref() {
            editor.replace_range(suggestion.replacement.clone(), &suggestion.insertion)?;
            editor.request_focus();
        }
        self.dismiss();
        Ok(())
    }
}

fn table(table: sql_bomb::Table) -> Table {
    Table {
        catalog: table.catalog,
        database_schema: table.database_schema,
        name: table.name,
        columns: table
            .schema
            .fields()
            .iter()
            .map(|field| Column {
                name: field.name().clone(),
                data_type: field.data_type().to_string(),
            })
            .collect(),
    }
}

impl Terminal {
    pub(super) fn query_focus(&self, event: &Event) -> Result<(), Error> {
        self.remember_focus(event);
        self.completion.editor.replace(Some(event.target.clone()));
        self.discover_schema()
    }

    pub(super) fn query_input(&self, event: &Event) -> Result<(), Error> {
        self.refresh_star()?;
        self.completion.refresh(&event.target);
        Ok(())
    }

    pub(super) fn refresh_schema(&self) -> Result<(), Error> {
        self.completion.reset();
        self.discover_schema()?;
        self.focus("query");
        Ok(())
    }

    fn discover_schema(&self) -> Result<(), Error> {
        self.connection.with(|connection| {
            if let Some(connection) = connection {
                self.completion
                    .load(connection.client.clone(), &self.services, &self.owner)?;
            }
            Ok(())
        })
    }

    pub(super) fn completion_key(&self, event: &Event) -> Result<(), Error> {
        let EventPayload::Input(Input::Key { key, modifiers, .. }) = event.payload else {
            return Ok(());
        };
        if modifiers.control || modifiers.alt || modifiers.super_key || modifiers.shift {
            return Ok(());
        }
        let count = self.completion.suggestions.with(Vec::len);
        if key == Key::Escape {
            self.completion.dismiss();
            if count > 0 {
                event.prevent_default();
            }
            return Ok(());
        }
        if count == 0 {
            return Ok(());
        }
        match key {
            Key::Up | Key::Down => {
                let direction = if key == Key::Up { -1 } else { 1 };
                self.completion.selected.update(|selected| {
                    *selected = selected.saturating_add_signed(direction).min(count - 1);
                });
                if let Some(list) = self.find_control("query-suggestions") {
                    list.scroll_to(0, self.completion.selected.get() as u16);
                }
            }
            Key::Tab => {
                let suggestion = self
                    .completion
                    .suggestions
                    .with(|suggestions| suggestions[self.completion.selected.get()].clone());
                self.completion.accept(&suggestion)?;
            }
            _ => return Ok(()),
        }
        event.prevent_default();
        Ok(())
    }
}
