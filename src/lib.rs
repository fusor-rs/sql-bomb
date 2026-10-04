mod clients;

use arrow_array::RecordBatch;
use arrow_schema::Schema;
use futures_util::{future::BoxFuture, stream::BoxStream};

pub use clients::FlightSql;

pub trait QueryClient {
    fn query(&self, sql: String) -> BoxFuture<'_, Result<Query, Error>>;

    /// `None` means discovery is unsupported; an empty result means no tables were found.
    fn tables(&self) -> Option<BoxFuture<'_, Result<Vec<Table>, Error>>> {
        None
    }
}

pub struct Query {
    pub schema: Schema,
    pub batches: BoxStream<'static, Result<RecordBatch, Error>>,
}

#[derive(Debug, PartialEq)]
pub struct Table {
    pub catalog: Option<String>,
    pub database_schema: Option<String>,
    pub name: String,
    pub table_type: String,
    pub schema: Schema,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid connection address; use http://host:port or https://host:port")]
    Address,
    #[error(
        "Use a server URL without a path, query, or credentials; put credentials in request headers"
    )]
    AddressCredentials,
    #[error("Invalid request headers; use one ASCII name: value per line (no binary headers)")]
    Headers,
    #[error("Cannot connect to the server; check its address and TLS configuration: {0}")]
    Connection(#[from] tonic::transport::Error),
    #[error("Flight SQL request failed; check the query and server: {0}")]
    Flight(#[from] arrow_flight::error::FlightError),
    #[error("Cannot decode the query result; check the server's Arrow response: {0}")]
    Arrow(#[from] arrow_schema::ArrowError),
    #[error("Invalid table metadata column {0}; check the server's table-discovery response")]
    TableMetadata(&'static str),
    #[error("Server returned an endpoint without a ticket; check the Flight SQL server")]
    MissingTicket,
}
