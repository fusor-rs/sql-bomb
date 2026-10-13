use sql_bomb::{Error, FlightSql, Postgres, QueryClient};
use std::rc::Rc;

pub(crate) const CLIENTS: &[Client] = &[Client::FlightSql, Client::Postgres];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Client {
    FlightSql,
    Postgres,
}

pub(crate) struct ClientProfile {
    pub(crate) name: &'static str,
    pub(crate) flag: &'static str,
    pub(crate) description: &'static str,
    pub(crate) default_address: &'static str,
    pub(crate) address_hint: &'static str,
    pub(crate) secret: SecretField,
}

pub(crate) struct SecretField {
    pub(crate) label: &'static str,
    pub(crate) noun: &'static str,
    pub(crate) multiline: bool,
    pub(crate) placeholder: &'static str,
    pub(crate) hint: &'static str,
}

pub(crate) struct Connection {
    pub(crate) kind: Client,
    pub(crate) address: String,
    pub(crate) secret: String,
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
                secret: SecretField {
                    label: "Request headers",
                    noun: "headers",
                    multiline: true,
                    placeholder: "authorization: Bearer …",
                    hint: "One name: value per line. Leave empty if not needed.",
                },
            },
            Self::Postgres => &ClientProfile {
                name: "PostgreSQL",
                flag: "--postgres",
                description: "Query a PostgreSQL server",
                default_address: "postgres://postgres@localhost:5432/postgres",
                address_hint: "Use postgres://user@host:port/database. sslmode works as in psql.",
                secret: SecretField {
                    label: "Password",
                    noun: "password",
                    multiline: false,
                    placeholder: "",
                    hint: "Leave empty if the server does not ask for one.",
                },
            },
        }
    }

    pub(crate) fn connect(self, address: String, secret: String) -> Result<Connection, Error> {
        let client: Rc<dyn QueryClient> = match self {
            Self::FlightSql => {
                let mut client = FlightSql::new(&address)?;
                client.set_headers(&secret)?;
                Rc::new(client)
            }
            Self::Postgres => Rc::new(Postgres::new(&address, &secret)?),
        };
        Ok(Connection {
            kind: self,
            address,
            secret,
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
