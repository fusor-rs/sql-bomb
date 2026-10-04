use arrow_array::{ArrayRef, Int64Array, RecordBatch, StringArray};
use arrow_flight::{
    FlightDescriptor, FlightEndpoint, FlightInfo, Ticket,
    encode::FlightDataEncoderBuilder,
    error::FlightError,
    flight_service_server::{FlightService, FlightServiceServer},
    sql::{
        CommandGetTables, CommandStatementQuery, ProstMessageExt, SqlInfo, TicketStatementQuery,
        server::FlightSqlService,
    },
};
use futures_util::{TryStreamExt, stream};
use prost::Message;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status, transport::Server};

const SAMPLE_QUERY: &str = "SELECT city, country, distance_km FROM destinations";

struct Destinations;

pub(super) async fn start() -> Result<String, Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = format!("http://{}", listener.local_addr()?);
    tokio::spawn(async move {
        if let Err(error) = Server::builder()
            .add_service(FlightServiceServer::new(Destinations))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
        {
            eprintln!("Preview Flight SQL server failed: {error}");
        }
    });
    Ok(address)
}

fn query(sql: &str) -> Result<RecordBatch, Box<dyn std::error::Error>> {
    let database = rusqlite::Connection::open_in_memory()?;
    database.execute_batch(include_str!("sample.sql"))?;
    let mut statement = database.prepare(sql)?;
    let mut cities = Vec::new();
    let mut countries = Vec::new();
    let mut distances = Vec::new();
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        cities.push(row.get::<_, String>(0)?);
        countries.push(row.get::<_, String>(1)?);
        distances.push(row.get::<_, i64>(2)?);
    }
    Ok(RecordBatch::try_from_iter([
        ("city", Arc::new(StringArray::from(cities)) as ArrayRef),
        (
            "country",
            Arc::new(StringArray::from(countries)) as ArrayRef,
        ),
        (
            "distance_km",
            Arc::new(Int64Array::from(distances)) as ArrayRef,
        ),
    ])?)
}

#[tonic::async_trait]
impl FlightSqlService for Destinations {
    type FlightService = Self;

    async fn get_flight_info_statement(
        &self,
        command: CommandStatementQuery,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        let batch =
            query(&command.query).map_err(|error| Status::invalid_argument(error.to_string()))?;
        flight_info(
            &batch,
            Ticket::new(
                TicketStatementQuery {
                    statement_handle: command.query.into(),
                }
                .as_any()
                .encode_to_vec(),
            ),
        )
    }

    async fn do_get_statement(
        &self,
        ticket: TicketStatementQuery,
        _request: Request<Ticket>,
    ) -> Result<Response<<Self as FlightService>::DoGetStream>, Status> {
        let sql = std::str::from_utf8(&ticket.statement_handle)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        let batch = query(sql).map_err(|error| Status::invalid_argument(error.to_string()))?;
        Ok(encode(batch))
    }

    async fn get_flight_info_tables(
        &self,
        command: CommandGetTables,
        _request: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        let ticket = Ticket::new(command.as_any().encode_to_vec());
        flight_info(&tables(command)?, ticket)
    }

    async fn do_get_tables(
        &self,
        command: CommandGetTables,
        _request: Request<Ticket>,
    ) -> Result<Response<<Self as FlightService>::DoGetStream>, Status> {
        Ok(encode(tables(command)?))
    }

    async fn register_sql_info(&self, _identifier: i32, _result: &SqlInfo) {}
}

fn tables(command: CommandGetTables) -> Result<RecordBatch, Status> {
    let batch = query(SAMPLE_QUERY).map_err(|error| Status::internal(error.to_string()))?;
    let mut tables = command.into_builder();
    tables.append("", "", "destinations", "TABLE", &batch.schema())?;
    tables
        .build()
        .map_err(|error| Status::internal(error.to_string()))
}

fn flight_info(batch: &RecordBatch, ticket: Ticket) -> Result<Response<FlightInfo>, Status> {
    Ok(Response::new(
        FlightInfo::new()
            .try_with_schema(&batch.schema())
            .map_err(FlightError::from)?
            .with_endpoint(FlightEndpoint::new().with_ticket(ticket)),
    ))
}

fn encode(batch: RecordBatch) -> Response<<Destinations as FlightService>::DoGetStream> {
    let encoded = FlightDataEncoderBuilder::new()
        .with_schema(batch.schema())
        .build(stream::iter([Ok(batch)]))
        .map_err(Status::from);
    Response::new(Box::pin(encoded))
}
