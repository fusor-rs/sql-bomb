use arrow_array::{ArrayRef, BinaryArray, RecordBatch, StringArray};
use arrow_flight::{
    FlightDescriptor, FlightEndpoint, FlightInfo, IpcMessage, SchemaAsIpc, Ticket,
    encode::FlightDataEncoderBuilder,
    error::FlightError,
    flight_service_server::{FlightService, FlightServiceServer},
    sql::{
        CommandGetTables, CommandStatementQuery, ProstMessageExt, SqlInfo, TicketStatementQuery,
        server::FlightSqlService,
    },
};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use futures_util::{StreamExt, TryStreamExt, stream, stream::BoxStream};
use prost::Message;
use sql_bomb::{Error, FlightSql, QueryClient, Table};
use std::sync::Arc;
use tokio::{
    net::TcpListener,
    sync::{Notify, oneshot},
};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status, transport::Server};

#[path = "support/completion.rs"]
mod completion;

const QUERY: &str = "SELECT city
FROM flights";
const EMPTY_QUERY: &str = "SELECT city
FROM flights
WHERE false";
const FAILED_QUERY: &str = "SELECT city
FROM missing_table";
const FIRST_TICKET: &[u8] = b"first";
const LAST_TICKET: &[u8] = b"last";
const PUBLIC_TICKET: &[u8] = b"public";
const HEADERS: &str = "authorization: Bearer fixture:token
x-tenant: tests";

struct Flights {
    schema: SchemaRef,
    release: Arc<Notify>,
    address: String,
}

#[tonic::async_trait]
impl FlightSqlService for Flights {
    type FlightService = Self;

    async fn get_flight_info_statement(
        &self,
        query: CommandStatementQuery,
        request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        authorize(&request)?;
        let mut flight = FlightInfo::new()
            .try_with_schema(&self.schema)
            .map_err(FlightError::from)?;
        match query.query.as_str() {
            QUERY => {
                flight = flight.with_endpoints(vec![
                    flight_endpoint(FIRST_TICKET),
                    flight_endpoint(LAST_TICKET).with_location(&self.address),
                    flight_endpoint(PUBLIC_TICKET)
                        .with_location(self.address.replace("127.0.0.1", "localhost")),
                ]);
            }
            EMPTY_QUERY => {}
            _ => return Err(Status::invalid_argument("Unknown table")),
        }
        Ok(Response::new(flight))
    }

    async fn do_get_statement(
        &self,
        ticket: TicketStatementQuery,
        request: Request<Ticket>,
    ) -> Result<Response<<Self as FlightService>::DoGetStream>, Status> {
        if ticket.statement_handle.as_ref() == PUBLIC_TICKET {
            if ["authorization", "x-tenant"]
                .iter()
                .any(|name| request.metadata().contains_key(*name))
            {
                return Err(Status::permission_denied("Headers crossed origins"));
            }
        } else {
            authorize(&request)?;
        }
        let batches = match ticket.statement_handle.as_ref() {
            FIRST_TICKET => {
                let first = cities(vec![Some("Bogotá"), None]);
                let release = self.release.clone();
                let remaining = async_stream::try_stream! {
                    release.notified().await;
                    yield cities(vec![Some("東京")]);
                };
                Box::pin(stream::once(async { Ok(first) }).chain(remaining))
                    as futures_util::stream::BoxStream<'static, Result<RecordBatch, FlightError>>
            }
            LAST_TICKET => Box::pin(stream::once(async { Ok(cities(vec![Some("Montréal")])) })),
            PUBLIC_TICKET => Box::pin(stream::empty()),
            _ => return Err(Status::not_found("Unknown ticket")),
        };
        Ok(encode_batches(self.schema.clone(), batches))
    }

    async fn get_flight_info_tables(
        &self,
        query: CommandGetTables,
        request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        authorize(&request)?;
        if query
            != (CommandGetTables {
                include_schema: true,
                ..Default::default()
            })
        {
            return Err(Status::invalid_argument("Expected all tables with schemas"));
        }
        let endpoints = [Some("warehouse"), None]
            .into_iter()
            .map(|catalog| {
                let ticket = CommandGetTables {
                    catalog: catalog.map(str::to_owned),
                    ..query.clone()
                };
                FlightEndpoint::new()
                    .with_ticket(Ticket::new(ticket.as_any().encode_to_vec()))
                    .with_location(&self.address)
            })
            .collect();
        let flight = FlightInfo::new()
            .try_with_schema(&query.into_builder().schema())
            .map_err(FlightError::from)?
            .with_endpoints(endpoints);
        Ok(Response::new(flight))
    }

    async fn do_get_tables(
        &self,
        query: CommandGetTables,
        request: Request<Ticket>,
    ) -> Result<Response<<Self as FlightService>::DoGetStream>, Status> {
        authorize(&request)?;
        let batch = table_batch(&self.schema);
        let batches = match query.catalog.as_deref() {
            Some("warehouse") => vec![Ok(batch.slice(0, 1)), Ok(batch.slice(1, 1))],
            None => vec![Ok(batch.slice(2, 1))],
            _ => return Err(Status::not_found("Unknown catalog ticket")),
        };
        Ok(encode_batches(
            batch.schema(),
            Box::pin(stream::iter(batches)),
        ))
    }

    async fn register_sql_info(&self, _identifier: i32, _result: &SqlInfo) {
        panic!("SQL metadata registration is not used by this fixture");
    }
}

fn encode_batches(
    schema: SchemaRef,
    batches: BoxStream<'static, Result<RecordBatch, FlightError>>,
) -> Response<<Flights as FlightService>::DoGetStream> {
    let encoded = FlightDataEncoderBuilder::new()
        .with_schema(schema)
        .build(batches)
        .map_err(Status::from);
    Response::new(Box::pin(encoded))
}

fn table_batch(schema: &Schema) -> RecordBatch {
    let columns = [
        (
            "catalog_name",
            vec![Some("warehouse"), Some("warehouse"), None],
            true,
        ),
        (
            "db_schema_name",
            vec![Some("travel"), Some("travel"), None],
            true,
        ),
        (
            "table_name",
            vec![Some("flights"), Some("arrivées"), Some("airports")],
            false,
        ),
        (
            "table_type",
            vec![Some("TABLE"), Some("VIEW"), Some("TABLE")],
            false,
        ),
    ]
    .into_iter()
    .map(|(name, values, nullable)| {
        (
            name,
            Arc::new(StringArray::from(values)) as ArrayRef,
            nullable,
        )
    });
    let serialized = IpcMessage::try_from(SchemaAsIpc::new(schema, &Default::default()))
        .expect("the fixture schema has an IPC representation");
    let schemas = BinaryArray::from(vec![serialized.0.as_ref(); 3]);
    RecordBatch::try_from_iter_with_nullable(columns.chain(std::iter::once((
        "table_schema",
        Arc::new(schemas) as ArrayRef,
        false,
    ))))
    .expect("table metadata columns have three rows with the declared types")
}

fn authorize<T>(request: &Request<T>) -> Result<(), Status> {
    for (name, value) in [
        ("authorization", "Bearer fixture:token"),
        ("x-tenant", "tests"),
    ] {
        if request
            .metadata()
            .get(name)
            .is_none_or(|header| header != value)
        {
            return Err(Status::unauthenticated("Missing request headers"));
        }
    }
    Ok(())
}

fn flight_endpoint(handle: &'static [u8]) -> FlightEndpoint {
    let ticket = TicketStatementQuery {
        statement_handle: handle.into(),
    };
    FlightEndpoint::new().with_ticket(Ticket::new(ticket.as_any().encode_to_vec()))
}

fn cities(values: Vec<Option<&str>>) -> RecordBatch {
    RecordBatch::try_from_iter_with_nullable([(
        "city",
        Arc::new(StringArray::from(values)) as ArrayRef,
        true,
    )])
    .expect("a single typed column defines a valid record batch")
}

#[tokio::test]
async fn streams_before_completion_and_reads_every_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("grpc+tcp://{}", listener.local_addr().unwrap());
    let release = Arc::new(Notify::new());
    let schema = Arc::new(Schema::new(vec![Field::new("city", DataType::Utf8, true)]));
    let flights = Flights {
        schema: schema.clone(),
        release: release.clone(),
        address: address.clone(),
    };
    let (shutdown, stopped) = oneshot::channel();
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(FlightServiceServer::new(flights))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                stopped.await.unwrap();
            })
            .await
            .unwrap();
    });
    let mut client = FlightSql::new(&address).unwrap();
    client.set_headers(HEADERS).unwrap();
    let client: &dyn QueryClient = &client;
    let mut query = client.query(QUERY.into()).await.unwrap();
    assert_eq!(query.schema, *schema);
    assert_eq!(
        query.batches.try_next().await.unwrap(),
        Some(cities(vec![Some("Bogotá"), None]))
    );
    release.notify_one();
    assert_eq!(
        query.batches.try_next().await.unwrap(),
        Some(cities(vec![Some("東京")]))
    );
    assert_eq!(
        query.batches.try_next().await.unwrap(),
        Some(cities(vec![Some("Montréal")]))
    );
    assert_eq!(query.batches.try_next().await.unwrap(), None);
    let mut empty = client.query(EMPTY_QUERY.into()).await.unwrap();
    assert_eq!(empty.schema, *schema);
    assert_eq!(empty.batches.try_next().await.unwrap(), None);
    let failure = client.query(FAILED_QUERY.into()).await.err().unwrap();
    let Error::Flight(FlightError::Tonic(status)) = failure else {
        panic!("Expected the server's Flight SQL error");
    };
    assert_eq!(status.code(), tonic::Code::InvalidArgument);
    assert_eq!(status.message(), "Unknown table");
    assert_table_discovery(client, &address).await;
    completion::exercise(&address).await;
    shutdown.send(()).unwrap();
    server.await.unwrap();
}

async fn assert_table_discovery(client: &dyn QueryClient, address: &str) {
    let tables = client
        .tables()
        .expect("Flight SQL supports table discovery")
        .await
        .expect("table discovery returns the fixture tables");
    let expected = [
        (Some("warehouse"), Some("travel"), "flights", "TABLE"),
        (Some("warehouse"), Some("travel"), "arrivées", "VIEW"),
        (None, None, "airports", "TABLE"),
    ]
    .into_iter()
    .map(|(catalog, database_schema, name, table_type)| Table {
        catalog: catalog.map(str::to_owned),
        database_schema: database_schema.map(str::to_owned),
        name: name.into(),
        table_type: table_type.into(),
        schema: Schema::new(vec![Field::new("city", DataType::Utf8, true)]),
    })
    .collect::<Vec<_>>();
    assert_eq!(tables, expected);
    let unauthorized = FlightSql::new(address).expect("the loopback server address is valid");
    let failure = unauthorized
        .tables()
        .expect("discovery is available before authentication")
        .await
        .expect_err("the server requires authentication headers");
    let Error::Flight(FlightError::Tonic(status)) = failure else {
        panic!("Expected the server's Flight SQL authentication error");
    };
    assert_eq!(status.code(), tonic::Code::Unauthenticated);
    assert_eq!(status.message(), "Missing request headers");
}
