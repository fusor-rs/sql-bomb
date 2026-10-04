use super::{Execution, Results, Screen, Terminal, Viewport};
use crate::client::{Client, Connection};
use crate::library::{Credentials, SavedConnection};
use fusor::{Signal, signal};
use hypercmd::Error;
use std::rc::Rc;

#[derive(Clone, Default, PartialEq)]
pub(super) enum ConnectionPurpose {
    #[default]
    New,
    Edit(Rc<SavedConnection>),
    Reopen(Rc<SavedConnection>),
    Current,
}

pub(super) struct ConnectionDraft {
    pub(super) name: Signal<String>,
    pub(super) save: Signal<bool>,
    pub(super) remember: Signal<bool>,
    pub(super) purpose: Signal<ConnectionPurpose>,
}

impl Default for ConnectionDraft {
    fn default() -> Self {
        Self {
            name: signal(String::new()),
            save: signal(false),
            remember: signal(false),
            purpose: signal(ConnectionPurpose::New),
        }
    }
}

impl Terminal {
    pub(super) fn select_client(&self, client: Client) {
        if self.address.with(|address| address.trim().is_empty()) {
            self.address.set(client.profile().default_address.into());
        }
        self.connection_error.set(String::new());
        self.screen.set(Screen::Connection(client));
        self.focus("address");
    }

    pub(super) fn saving_connection(&self) -> bool {
        self.draft.save.get() || !matches!(self.draft.purpose.get(), ConnectionPurpose::New)
    }

    pub(super) fn suggest_connection_name(&self) {
        if self.draft.name.with(|name| name.trim().is_empty()) {
            let name = self.address.with(|address| {
                // A partially typed address has no hostname to suggest yet.
                address
                    .parse::<tonic::codegen::http::Uri>()
                    .map_or_else(|_| String::new(), |uri| uri.host().unwrap_or("").into())
            });
            self.draft.name.set(name);
        }
    }

    pub(super) fn connect(&self) -> Result<(), Error> {
        let headers_missing = self
            .request_headers
            .with(|headers| headers.trim().is_empty());
        if self.saving_connection() && self.draft.remember.get() && headers_missing {
            self.connection_error
                .set("Enter headers to remember, or uncheck Remember headers.".into());
            return Ok(());
        }
        if matches!(self.screen.get(), Screen::Credentials(_)) && headers_missing {
            self.connection_error
                .set("Enter the request headers for this connection.".into());
            return Ok(());
        }
        let kind = match self.screen.get() {
            Screen::Connection(kind) => kind,
            Screen::Credentials(profile) => profile.kind,
            _ => return Ok(()),
        };
        let address = self.address.with(|address| address.trim().to_owned());
        let connection = kind.connect(address, self.request_headers.get());
        match connection {
            Ok(connection) => {
                if let Err(error) = self.submit_connection(connection) {
                    self.connection_error.set(error.to_string());
                }
            }
            Err(error) => self.connection_error.set(error.to_string()),
        }
        Ok(())
    }

    fn submit_connection(&self, mut connection: Connection) -> Result<(), Error> {
        if !self.saving_connection() {
            return self.activate_connection(connection);
        }
        let original = match self.draft.purpose.get() {
            ConnectionPurpose::Edit(profile) | ConnectionPurpose::Reopen(profile) => Some(profile),
            _ => None,
        };
        self.suggest_connection_name();
        let name = self.draft.name.with(|name| name.trim().to_owned());
        let credentials = if connection.headers.trim().is_empty() {
            original
                .as_ref()
                .filter(|profile| profile.credentials == Credentials::Session)
                .map_or(Credentials::None, |_| Credentials::Session)
        } else if self.draft.remember.get() {
            Credentials::Keyring
        } else {
            Credentials::Session
        };
        let saved = self.library.save_connection(
            original.as_ref().map(|profile| &profile.id),
            &name,
            &connection,
            credentials,
        )?;
        connection.saved = Some(saved.clone());
        match self.draft.purpose.get() {
            ConnectionPurpose::Edit(_) => {
                self.update_connection_profile(saved);
                self.change_connection()
            }
            ConnectionPurpose::Current => {
                self.library.opened_connection(&saved.id)?;
                self.connection.update(|active| {
                    active
                        .as_mut()
                        .expect("saving the current connection requires a workspace")
                        .saved = Some(saved);
                });
                self.screen.set(Screen::Workspace);
                self.focus("query");
                Ok(())
            }
            _ => {
                self.draft.purpose.set(ConnectionPurpose::Reopen(saved));
                self.activate_connection(connection)
            }
        }
    }

    fn update_connection_profile(&self, profile: Rc<SavedConnection>) {
        self.connection.update(|active| {
            let Some(active) = active else {
                return;
            };
            if active
                .saved
                .as_ref()
                .is_some_and(|saved| saved.id == profile.id)
            {
                active.saved = (active.address == profile.address).then_some(profile);
            }
        });
    }

    pub(super) fn activate_connection(&self, connection: Connection) -> Result<(), Error> {
        if let Some(profile) = &connection.saved {
            self.library.opened_connection(&profile.id)?;
        }
        self.completion.reset();
        self.connection.replace(Some(connection));
        self.connection_error.set(String::new());
        self.results.replace(Results::default());
        self.viewport.update(Viewport::reset);
        self.execution.replace(Execution::Idle);
        self.screen.set(Screen::Workspace);
        self.refresh_star()?;
        self.focus("query");
        Ok(())
    }

    pub(super) fn save_current_connection(&self) {
        self.connection.with(|connection| {
            if let Some(connection) = connection {
                self.address.set(connection.address.clone());
                self.request_headers.set(connection.headers.clone());
                self.draft.name.set(String::new());
                self.draft.save.set(true);
                self.draft.remember.set(false);
                self.draft.purpose.set(ConnectionPurpose::Current);
                self.connection_error.set(String::new());
                self.screen.set(Screen::Connection(connection.kind));
                self.suggest_connection_name();
                self.focus("connection-name");
            }
        });
    }

    pub(super) fn connection_label(&self) -> String {
        self.connection.with(|connection| match connection {
            Some(connection) => {
                let destination = format!(
                    "{} · {}",
                    connection.kind.profile().name,
                    connection.address
                );
                match &connection.saved {
                    Some(profile) => format!("{} · {destination}", profile.name),
                    None => destination,
                }
            }
            None if self.screen.get() == Screen::Connections => {
                "Open a saved connection or create one".into()
            }
            None => "Choose a client to get started".into(),
        })
    }

    pub(super) fn connection_action(&self) -> &'static str {
        match self.draft.purpose.get() {
            ConnectionPurpose::Current => "Save connection",
            ConnectionPurpose::Edit(_) => "Save changes",
            _ if self.saving_connection() => "Save & open →",
            _ => "Open workspace →",
        }
    }

    pub(super) fn leave_connection(&self) -> Result<(), Error> {
        match self.screen.get() {
            Screen::Connection(_) if matches!(self.draft.purpose.get(), ConnectionPurpose::New) => {
                self.choose_client();
            }
            Screen::Connection(_)
                if matches!(self.draft.purpose.get(), ConnectionPurpose::Current) =>
            {
                self.screen.set(Screen::Workspace);
                self.focus("query");
            }
            Screen::Connection(_) | Screen::Credentials(_) | Screen::RemoveConnection(_) => {
                self.change_connection()?;
            }
            Screen::Clients if !self.catalog.entries.with(Vec::is_empty) => {
                self.change_connection()?;
            }
            Screen::Clients | Screen::Connections if self.connection.with(Option::is_some) => {
                self.screen.set(Screen::Workspace);
                self.focus("query");
            }
            _ => {}
        }
        Ok(())
    }
}
