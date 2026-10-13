mod columns;
mod tables;
mod tls;
mod values;

use crate::{Error, Query, QueryClient, Table};
use columns::Columns;
use futures_util::{StreamExt, future::BoxFuture};
use std::{iter, pin::pin};
use tokio::task::JoinSet;
use tokio_postgres::{Client, Config, types::ToSql};
use tokio_postgres_rustls::MakeRustlsConnect;

const BATCH_ROWS: usize = 1024;

pub struct Postgres {
    config: Config,
    tls: MakeRustlsConnect,
}

struct Session {
    client: Client,
    // Dropping the set aborts the connection task, closing the socket when a query stream is
    // dropped. Connection failures surface as closed-connection errors on the pending request.
    _connection: JoinSet<Result<(), tokio_postgres::Error>>,
}

impl Postgres {
    /// An empty `password` sends none.
    pub fn new(address: &str, password: &str) -> Result<Self, Error> {
        let (address, tls) = tls::connector(address)?;
        let mut config: Config = address.parse().map_err(Error::PostgresAddress)?;
        if config.get_password().is_some() {
            return Err(Error::PostgresPassword);
        }
        if !password.is_empty() {
            config.password(password);
        }
        Ok(Self { config, tls })
    }

    async fn connect(&self) -> Result<Session, Error> {
        let (client, connection) = self.config.connect(self.tls.clone()).await?;
        let mut tasks = JoinSet::new();
        tasks.spawn(connection);
        Ok(Session {
            client,
            _connection: tasks,
        })
    }
}

impl QueryClient for Postgres {
    fn query(&self, sql: String) -> BoxFuture<'_, Result<Query, Error>> {
        Box::pin(async move {
            let session = self.connect().await?;
            let statement = session.client.prepare(&sql).await?;
            let columns = Columns::new(statement.columns())?;
            let schema = columns.schema.as_ref().clone();
            let rows = session
                .client
                .query_raw(&statement, iter::empty::<&dyn ToSql>())
                .await?;
            let batches = async_stream::try_stream! {
                let mut chunks = pin!(rows.ready_chunks(BATCH_ROWS));
                while let Some(chunk) = chunks.next().await {
                    let rows = chunk.into_iter().collect::<Result<Vec<_>, _>>()?;
                    yield columns.batch(&rows)?;
                }
                // The stream owns the session so the connection stays open until rows end.
                drop(session);
            };
            Ok(Query {
                schema,
                batches: Box::pin(batches),
            })
        })
    }

    fn tables(&self) -> Option<BoxFuture<'_, Result<Vec<Table>, Error>>> {
        Some(Box::pin(async move {
            let session = self.connect().await?;
            let rows = session.client.query(tables::QUERY, &[]).await?;
            rows.iter().map(tables::table).collect()
        }))
    }
}
