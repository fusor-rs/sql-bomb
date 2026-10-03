mod connections;
mod credentials;

pub(crate) use connections::{Credentials, SavedConnection};

use rusqlite::{Connection, params};
use std::{
    fs::{DirBuilder, OpenOptions},
    ops::Range,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
    rc::Rc,
    time::Duration,
};

pub(crate) const DIRECTORY: &str = ".sqlbomb";
const DATABASE: &str = "queries.sqlite3";
const PRIVATE_DIRECTORY: u32 = 0o700;
const PRIVATE_FILE: u32 = 0o600;
const FILTER: &str = "
    FROM library_entries
    WHERE starred_only = ?1
      AND (instr(lower(sql), lower(?2)) > 0 OR instr(lower(endpoint), lower(?2)) > 0)
";

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Cannot access the query library; check its permissions and free disk space: {0}")]
    Filesystem(#[from] std::io::Error),
    #[error("Cannot read or save the query library; check its database and file permissions: {0}")]
    Database(#[from] rusqlite::Error),
    #[error(
        "Cannot access saved credentials; unlock your OS credential store or use session-only headers"
    )]
    Credentials(#[from] keyring::Error),
    #[error(
        "Connection changes could not be saved or credentials restored; review this connection's settings"
    )]
    CredentialRollback,
    #[error("Saved connection uses an unavailable client: {0}")]
    Client(String),
    #[error("This connection was removed in another session; return to Connections")]
    MissingConnection,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Collection {
    History,
    Starred,
}

#[derive(Clone, Copy)]
pub(crate) enum Change {
    Executed,
    ToggleStar,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EntryId(i64);

#[derive(Clone, PartialEq)]
pub(crate) struct Entry {
    pub(crate) id: EntryId,
    pub(crate) endpoint: String,
    pub(crate) sql: String,
    pub(crate) starred: bool,
    recorded_at: String,
}

impl Entry {
    pub(crate) fn context(&self) -> String {
        format!("{} · {}", self.recorded_at, self.endpoint)
    }
}

pub(crate) struct Library {
    connection: Connection,
    pub(crate) credentials: Box<credentials::CredentialStore>,
}

impl Library {
    pub(crate) fn open(directory: &Path) -> Result<Self, Error> {
        DirBuilder::new()
            .recursive(true)
            .mode(PRIVATE_DIRECTORY)
            .create(directory)?;
        let path = directory.join(DATABASE);
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(PRIVATE_FILE)
            .open(&path)?;
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::ZERO)?;
        connection.execute_batch(include_str!("library/schema.sql"))?;
        Ok(Self {
            connection,
            credentials: Box::new(|account| Ok(Rc::new(keyring::Entry::new("sql-bomb", account)?))),
        })
    }

    pub(crate) fn save(&self, endpoint: &str, sql: &str, change: Change) -> Result<(), Error> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO queries (endpoint, sql)
             VALUES (?1, ?2) ON CONFLICT DO NOTHING",
            params![endpoint, sql],
        )?;
        let statement = match change {
            Change::Executed => {
                "
                INSERT INTO history (query_id)
                SELECT id FROM queries WHERE endpoint = ?1 AND sql = ?2
            "
            }
            Change::ToggleStar => {
                "
                UPDATE queries
                SET starred_at = CASE WHEN starred_at IS NULL THEN unixepoch() ELSE NULL END
                WHERE endpoint = ?1 AND sql = ?2
            "
            }
        };
        transaction.execute(statement, params![endpoint, sql])?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn starred(&self, endpoint: &str, sql: &str) -> Result<bool, Error> {
        Ok(self
            .connection
            .prepare_cached(
                "
            SELECT EXISTS (
                SELECT 1 FROM queries WHERE endpoint = ?1 AND sql = ?2 AND starred_at IS NOT NULL
            )
        ",
            )?
            .query_row(params![endpoint, sql], |row| row.get(0))?)
    }

    pub(crate) fn count(&self, collection: Collection, search: &str) -> Result<usize, Error> {
        Ok(self
            .connection
            .prepare_cached(&format!("SELECT count(*) {FILTER}"))?
            .query_row(params![collection == Collection::Starred, search], |row| {
                row.get(0)
            })?)
    }

    pub(crate) fn entries(
        &self,
        collection: Collection,
        search: &str,
        range: Range<usize>,
    ) -> Result<Vec<Rc<Entry>>, Error> {
        let mut statement = self.connection.prepare_cached(&format!(
            "
            SELECT id, endpoint, sql, starred,
                   strftime('%Y-%m-%d %H:%M:%S', recorded_at, 'unixepoch', 'localtime')
            {FILTER}
            ORDER BY recorded_at DESC, id DESC
            LIMIT ?3 OFFSET ?4
        "
        ))?;
        let entries = statement.query_map(
            params![
                collection == Collection::Starred,
                search,
                range.len(),
                range.start
            ],
            |row| {
                Ok(Rc::new(Entry {
                    id: EntryId(row.get(0)?),
                    endpoint: row.get(1)?,
                    sql: row.get(2)?,
                    starred: row.get(3)?,
                    recorded_at: row.get(4)?,
                }))
            },
        )?;
        Ok(entries.collect::<Result<_, _>>()?)
    }
}
