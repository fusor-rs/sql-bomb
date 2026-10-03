use super::{Screen, Terminal, connection::ConnectionPurpose};
use crate::library::{Credentials, Library, SavedConnection};
use fusor::{Signal, signal};
use hypercmd::{Error, Event, EventPayload, Input, Key};
use std::rc::Rc;

const CONNECTION_ROW_HEIGHT: usize = 3;

pub(super) struct ConnectionCatalog {
    pub(super) entries: Signal<Vec<Rc<SavedConnection>>>,
    pub(super) search: Signal<String>,
    pub(super) selected: Signal<usize>,
}

impl ConnectionCatalog {
    pub(super) fn load(library: &Library) -> Result<Self, Error> {
        Ok(Self {
            entries: signal(library.connections("")?),
            search: signal(String::new()),
            selected: signal(0),
        })
    }
}

impl Terminal {
    pub(super) fn change_connection(&self) -> Result<(), Error> {
        self.catalog.entries.set(self.library.connections("")?);
        self.catalog.search.set(String::new());
        self.catalog.selected.set(0);
        self.connection_error.set(String::new());
        self.menu.replace(None);
        if self.catalog.entries.with(Vec::is_empty) {
            self.new_connection()?;
        } else {
            self.screen.set(Screen::Connections);
            self.focus("connection-search");
        }
        Ok(())
    }

    pub(super) fn new_connection(&self) -> Result<(), Error> {
        self.catalog.search.set(String::new());
        self.search_connections()?;
        self.address.set(String::new());
        self.request_headers.set(String::new());
        self.draft.name.set(String::new());
        self.draft.save.set(false);
        self.draft.remember.set(false);
        self.draft.purpose.set(ConnectionPurpose::New);
        self.choose_client();
        Ok(())
    }

    pub(super) fn choose_client(&self) {
        self.connection_error.set(String::new());
        self.screen.set(Screen::Clients);
    }

    pub(super) fn search_connections(&self) -> Result<(), Error> {
        let entries = self
            .catalog
            .search
            .with(|search| self.library.connections(search))?;
        self.catalog.entries.set(entries);
        self.catalog.selected.set(0);
        Ok(())
    }

    pub(super) fn selected_connection(&self) -> Option<Rc<SavedConnection>> {
        self.catalog
            .entries
            .with(|entries| entries.get(self.catalog.selected.get()).cloned())
    }

    pub(super) fn select_connection(&self, profile: &SavedConnection) {
        self.catalog.entries.with(|entries| {
            if let Some(index) = entries.iter().position(|entry| entry.id == profile.id) {
                self.catalog.selected.set(index);
            }
        });
    }

    pub(super) fn open_saved_connection(&self, profile: Rc<SavedConnection>) -> Result<(), Error> {
        self.connection_error.set(String::new());
        match self.library.connection_headers(&profile) {
            Ok(Some(headers)) => match profile.kind.connect(profile.address.clone(), headers) {
                Ok(mut connection) => {
                    connection.saved = Some(profile);
                    self.activate_connection(connection)?;
                }
                Err(error) => self.connection_error.set(error.to_string()),
            },
            Ok(None) => {
                self.prepare_saved_connection(&profile, String::new());
                self.draft
                    .purpose
                    .set(ConnectionPurpose::Reopen(profile.clone()));
                self.screen.set(Screen::Credentials(profile));
                self.focus("request-headers");
            }
            Err(error) => self.connection_error.set(error.to_string()),
        }
        Ok(())
    }

    pub(super) fn open_selected_connection(&self) -> Result<(), Error> {
        if let Some(profile) = self.selected_connection() {
            self.open_saved_connection(profile)?;
        }
        Ok(())
    }

    pub(super) fn edit_connection(&self) -> Result<(), Error> {
        let Some(profile) = self.selected_connection() else {
            return Ok(());
        };
        match self.library.connection_headers(&profile) {
            Ok(headers) => {
                self.prepare_saved_connection(&profile, headers.unwrap_or_default());
                self.screen.set(Screen::Connection(profile.kind));
                self.focus("connection-name");
            }
            Err(error) => self.connection_error.set(error.to_string()),
        }
        Ok(())
    }

    fn prepare_saved_connection(&self, profile: &Rc<SavedConnection>, headers: String) {
        self.address.set(profile.address.clone());
        self.request_headers.set(headers);
        self.draft.name.set(profile.name.clone());
        self.draft.save.set(true);
        self.draft
            .remember
            .set(profile.credentials == Credentials::Keyring);
        self.draft
            .purpose
            .set(ConnectionPurpose::Edit(profile.clone()));
        self.connection_error.set(String::new());
    }

    pub(super) fn confirm_remove_connection(&self) {
        if let Some(profile) = self.selected_connection() {
            self.connection_error.set(String::new());
            self.screen.set(Screen::RemoveConnection(profile));
            self.focus("keep-connection");
        }
    }

    pub(super) fn removing_connection_name(&self) -> String {
        match self.screen.get() {
            Screen::RemoveConnection(profile) => profile.name.clone(),
            _ => String::new(),
        }
    }

    pub(super) fn remove_connection(&self) -> Result<(), Error> {
        let Screen::RemoveConnection(profile) = self.screen.get() else {
            return Ok(());
        };
        if let Err(error) = self.library.remove_connection(&profile) {
            self.connection_error.set(error.to_string());
            return Ok(());
        }
        self.connection.update(|active| {
            let Some(active) = active else {
                return;
            };
            if active
                .saved
                .as_ref()
                .is_some_and(|saved| saved.id == profile.id)
            {
                active.saved = None;
            }
        });
        self.change_connection()?;
        Ok(())
    }

    pub(super) fn connections_navigation(&self, event: &Event) -> Result<(), Error> {
        let EventPayload::Input(Input::Key { key, modifiers, .. }) = event.payload else {
            return Ok(());
        };
        if modifiers.control || modifiers.alt || modifiers.super_key {
            return Ok(());
        }
        match key {
            Key::Enter if event.target.tag() == "input" => {
                self.open_selected_connection()?;
            }
            Key::Up | Key::Down => {
                let last = self
                    .catalog
                    .entries
                    .with(|entries| entries.len().saturating_sub(1));
                let direction = if key == Key::Up { -1 } else { 1 };
                self.catalog.selected.update(|selected| {
                    *selected = selected.saturating_add_signed(direction).min(last);
                });
                self.focus("connection-search");
                if let Some(list) = self.find_control("saved-connections") {
                    list.scroll_to(
                        0,
                        (self.catalog.selected.get() * CONNECTION_ROW_HEIGHT) as u16,
                    );
                }
            }
            _ => return Ok(()),
        }
        event.prevent_default();
        Ok(())
    }
}
