use super::{
    Error, Library,
    credentials::{self, CredentialChange},
};
use crate::client::{CLIENTS, Connection};
use rusqlite::{OptionalExtension, params};
use std::rc::Rc;

const CONNECTION_ID_BYTES: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ConnectionId(pub(crate) String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Credentials {
    None,
    Session,
    Keyring,
}

impl Credentials {
    fn stored(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Session => "session",
            Self::Keyring => "keyring",
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct SavedConnection {
    pub(crate) id: ConnectionId,
    pub(crate) name: String,
    pub(crate) kind: crate::client::Client,
    pub(crate) address: String,
    pub(crate) credentials: Credentials,
}

impl Library {
    pub(crate) fn connections(&self, search: &str) -> Result<Vec<Rc<SavedConnection>>, Error> {
        let mut statement = self.connection.prepare_cached(
            "
            SELECT id, name, client, address, credentials
            FROM saved_connections
            WHERE instr(lower(name || ' ' || client || ' ' || address), lower(?1)) > 0
            ORDER BY opened_at DESC, name COLLATE NOCASE, id
        ",
        )?;
        let rows = statement.query_map([search], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get(1)?,
                row.get::<_, String>(2)?,
                row.get(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, client, address, credentials) = row?;
            let kind = CLIENTS
                .iter()
                .find(|kind| kind.profile().flag == client)
                .copied()
                .ok_or(Error::Client(client))?;
            let credentials = [
                Credentials::None,
                Credentials::Session,
                Credentials::Keyring,
            ]
            .into_iter()
            .find(|mode| mode.stored() == credentials)
            .expect("saved credential modes are constrained by the schema");
            Ok(Rc::new(SavedConnection {
                id: ConnectionId(id),
                name,
                kind,
                address,
                credentials,
            }))
        })
        .collect()
    }

    pub(crate) fn save_connection(
        &self,
        original: Option<&ConnectionId>,
        name: &str,
        connection: &Connection,
        credentials: Credentials,
    ) -> Result<Rc<SavedConnection>, Error> {
        let id = match original {
            Some(id) => id.clone(),
            None => ConnectionId(self.connection.query_row(
                "SELECT lower(hex(randomblob(?1)))",
                [CONNECTION_ID_BYTES],
                |row| row.get(0),
            )?),
        };
        let transaction = self.connection.unchecked_transaction()?;
        let previous = stored_credentials(&transaction, &id)?;
        if original.is_some() && previous.is_none() {
            return Err(Error::MissingConnection);
        }
        transaction.execute(
            "
            INSERT INTO saved_connections (id, name, client, address, credentials)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET name = excluded.name, client = excluded.client,
                address = excluded.address, credentials = excluded.credentials
        ",
            params![
                id.0,
                name,
                connection.kind.profile().flag,
                connection.address,
                credentials.stored()
            ],
        )?;
        let change = if credentials == Credentials::Keyring {
            CredentialChange::Store(&connection.headers)
        } else if previous.as_deref() == Some(Credentials::Keyring.stored()) {
            CredentialChange::Forget
        } else {
            CredentialChange::Keep
        };
        self.commit_credentials(transaction, &id.0, change)?;
        Ok(Rc::new(SavedConnection {
            id,
            name: name.into(),
            kind: connection.kind,
            address: connection.address.clone(),
            credentials,
        }))
    }

    pub(crate) fn connection_headers(
        &self,
        profile: &SavedConnection,
    ) -> Result<Option<String>, Error> {
        match profile.credentials {
            Credentials::None => Ok(Some(String::new())),
            Credentials::Session => Ok(None),
            Credentials::Keyring => {
                let entry = (self.credentials)(&profile.id.0)?;
                Ok(credentials::read(&entry)?)
            }
        }
    }

    pub(crate) fn opened_connection(&self, id: &ConnectionId) -> Result<(), Error> {
        let changed = self.connection.execute(
            "
            UPDATE saved_connections
            SET opened_at = (SELECT coalesce(max(opened_at), 0) + 1 FROM saved_connections)
            WHERE id = ?1
        ",
            [&id.0],
        )?;
        if changed == 0 {
            return Err(Error::MissingConnection);
        }
        Ok(())
    }

    pub(crate) fn remove_connection(&self, profile: &SavedConnection) -> Result<(), Error> {
        let transaction = self.connection.unchecked_transaction()?;
        let previous = stored_credentials(&transaction, &profile.id)?;
        transaction.execute(
            "DELETE FROM saved_connections WHERE id = ?1",
            [&profile.id.0],
        )?;
        let change = if previous.as_deref() == Some(Credentials::Keyring.stored()) {
            CredentialChange::Forget
        } else {
            CredentialChange::Keep
        };
        self.commit_credentials(transaction, &profile.id.0, change)
    }
}

fn stored_credentials(
    connection: &rusqlite::Connection,
    id: &ConnectionId,
) -> Result<Option<String>, Error> {
    Ok(connection
        .query_row(
            "SELECT credentials FROM saved_connections WHERE id = ?1",
            [&id.0],
            |row| row.get(0),
        )
        .optional()?)
}
