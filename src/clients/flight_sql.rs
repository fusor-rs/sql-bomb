use crate::{Error, Query, QueryClient};
use arrow_flight::{
    FlightEndpoint, decode::FlightRecordBatchStream, flight_service_client::FlightServiceClient,
    sql::client::FlightSqlServiceClient,
};
use futures_util::{TryStreamExt, future::BoxFuture};
use std::collections::BTreeMap;
use tonic::{
    metadata::{Ascii, MetadataKey, MetadataValue},
    transport::{Channel, ClientTlsConfig, Endpoint},
};

const REUSE_CONNECTION: &str = "arrow-flight-reuse-connection://?";
const GRPC_PLAINTEXT: &str = "grpc+tcp://";
const GRPC_TLS: &str = "grpc+tls://";
const HTTP: &str = "http://";
const HTTPS_SCHEME: &str = "https";

pub struct FlightSql {
    endpoint: Endpoint,
    headers: BTreeMap<String, String>,
}

impl FlightSql {
    pub fn new(address: &str) -> Result<Self, Error> {
        Ok(Self {
            endpoint: endpoint(address)?,
            headers: BTreeMap::new(),
        })
    }

    pub fn set_headers(&mut self, headers: &str) -> Result<(), Error> {
        let mut validated = BTreeMap::new();
        for line in headers.lines().filter(|line| !line.trim().is_empty()) {
            let (name, value) = line.split_once(':').ok_or(Error::Headers)?;
            let name = MetadataKey::<Ascii>::from_bytes(name.trim().as_bytes())
                .map_err(|_| Error::Headers)?;
            let value = value.trim();
            value
                .parse::<MetadataValue<Ascii>>()
                .map_err(|_| Error::Headers)?;
            validated.insert(name.as_str().to_owned(), value.to_owned());
        }
        self.headers = validated;
        Ok(())
    }
}

impl QueryClient for FlightSql {
    fn query(&self, sql: String) -> BoxFuture<'_, Result<Query, Error>> {
        Box::pin(async move {
            let mut client = FlightSqlServiceClient::new(self.endpoint.connect().await?);
            for (name, value) in &self.headers {
                client.set_header(name, value);
            }
            let mut flight = client.execute(sql, None).await?;
            let endpoints = std::mem::take(&mut flight.endpoint);
            let schema = flight.try_decode_schema()?;
            let origin = self.endpoint.clone();
            let batches = async_stream::try_stream! {
                for endpoint in endpoints {
                    let mut batches = fetch_endpoint(&mut client, endpoint, &origin).await?;
                    while let Some(batch) = batches.try_next().await? {
                        yield batch;
                    }
                }
            };
            Ok(Query {
                schema,
                batches: Box::pin(batches),
            })
        })
    }
}

fn endpoint(address: &str) -> Result<Endpoint, Error> {
    let address = if let Some(authority) = address.strip_prefix(GRPC_PLAINTEXT) {
        format!("{HTTP}{authority}")
    } else if let Some(authority) = address.strip_prefix(GRPC_TLS) {
        format!("{HTTPS_SCHEME}://{authority}")
    } else {
        address.to_owned()
    };
    let mut endpoint = Endpoint::from_shared(address).map_err(|_| Error::Address)?;
    if !matches!(endpoint.uri().scheme_str(), Some("http" | "https"))
        || endpoint.uri().authority().is_none()
    {
        return Err(Error::Address);
    }
    if !matches!(endpoint.uri().path(), "" | "/")
        || endpoint.uri().query().is_some()
        || endpoint
            .uri()
            .authority()
            .is_some_and(|authority| authority.as_str().contains('@'))
    {
        return Err(Error::AddressCredentials);
    }
    if endpoint.uri().scheme_str() == Some(HTTPS_SCHEME) {
        endpoint = endpoint.tls_config(ClientTlsConfig::new().with_native_roots())?;
    }
    Ok(endpoint)
}

async fn fetch_endpoint(
    client: &mut FlightSqlServiceClient<Channel>,
    flight: FlightEndpoint,
    origin: &Endpoint,
) -> Result<FlightRecordBatchStream, Error> {
    let ticket = flight.ticket.ok_or(Error::MissingTicket)?;
    match flight.location.first() {
        Some(location) if location.uri != REUSE_CONNECTION => {
            let endpoint = endpoint(&location.uri)?;
            let channel = endpoint.connect().await?;
            let mut remote = if endpoint.uri().scheme() == origin.uri().scheme()
                && endpoint.uri().authority() == origin.uri().authority()
            {
                let mut remote = client.clone();
                *remote.inner_mut() = FlightServiceClient::new(channel);
                remote
            } else {
                FlightSqlServiceClient::new(channel)
            };
            Ok(remote.do_get(ticket).await?)
        }
        _ => Ok(client.do_get(ticket).await?),
    }
}
