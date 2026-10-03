use sql_bomb::{Error, FlightSql, QueryClient};
use std::rc::Rc;

pub(crate) const CLIENTS: &[Client] = &[Client::FlightSql];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Client {
    FlightSql,
}

pub(crate) struct ClientProfile {
    pub(crate) name: &'static str,
    pub(crate) flag: &'static str,
    pub(crate) description: &'static str,
    pub(crate) default_address: &'static str,
    pub(crate) address_hint: &'static str,
    pub(crate) supports_headers: bool,
}

pub(crate) struct Connection {
    pub(crate) kind: Client,
    pub(crate) address: String,
    pub(crate) headers: String,
    pub(crate) client: Rc<dyn QueryClient>,
    pub(crate) saved: Option<Rc<crate::library::SavedConnection>>,
}

impl Client {
    pub(crate) fn profile(self) -> &'static ClientProfile {
        match self {
            Self::FlightSql => &ClientProfile {
                name: "Flight SQL",
                flag: "--flightsql",
                description: "Stream Arrow results over gRPC",
                default_address: "http://localhost:50051",
                address_hint: "Use http://host:port or https://host:port for TLS.",
                supports_headers: true,
            },
        }
    }

    pub(crate) fn connect(self, address: String, headers: String) -> Result<Connection, Error> {
        let client: Rc<dyn QueryClient> = match self {
            Self::FlightSql => {
                let mut client = FlightSql::new(&address)?;
                client.set_headers(&headers)?;
                Rc::new(client)
            }
        };
        Ok(Connection {
            kind: self,
            address,
            headers,
            client,
            saved: None,
        })
    }
}

pub(crate) enum Startup {
    Choose(Option<String>),
    Configure(Client),
    Connected(Connection),
}
