mod completion;
mod connection;
mod connections;
mod inspection;
mod menu;
mod navigation;
mod recall;
mod view;
mod welcome;
use completion::Completion;
use connection::ConnectionDraft;
use connections::ConnectionCatalog;
use inspection::Inspection;
use menu::Menu;
use recall::Browser;

use crate::client::{Client, Connection, Startup};
use crate::library::{Change, Library, SavedConnection};
use crate::results::{Cell, Results, Row, Viewport};
use fusor::{FromInputs, OwnerHandle, Signal, signal};
use futures_util::{
    TryStreamExt,
    future::{AbortHandle, Abortable},
};
use hypercmd::{Error, Node, Services};
use sql_bomb::QueryClient;
use std::{cell::RefCell, rc::Rc, time::Instant};

#[derive(Clone, PartialEq)]
pub(crate) enum Screen {
    Clients,
    Connections,
    Connection(Client),
    Credentials(Rc<SavedConnection>),
    RemoveConnection(Rc<SavedConnection>),
    Workspace,
    Library,
    Inspector(Inspection),
}

enum Execution {
    Idle,
    Running(AbortHandle),
    Finished(String),
    Cancelled,
    Failed(String),
}

pub(crate) struct Terminal {
    connection: Signal<Option<Connection>>,
    address: Signal<String>,
    request_headers: Signal<String>,
    connection_error: Signal<String>,
    catalog: ConnectionCatalog,
    draft: ConnectionDraft,
    screen: Signal<Screen>,
    owner: OwnerHandle,
    services: Services,
    sql: Signal<String>,
    completion: Rc<Completion>,
    execution: Signal<Execution>,
    results: Signal<Results>,
    viewport: Signal<Viewport>,
    library: Library,
    browser: Signal<Browser>,
    search: Signal<String>,
    starred: Signal<bool>,
    narrow_library: Signal<bool>,
    menu: Signal<Option<Menu>>,
    menu_search: Signal<String>,
    content_focus: RefCell<Option<Node>>,
    root: RefCell<Option<Node>>,
}

impl FromInputs for Terminal {
    type Inputs = (Startup, Library);
    type Error = Error;

    fn from_inputs((startup, library): Self::Inputs, owner: OwnerHandle) -> Result<Self, Error> {
        let catalog = ConnectionCatalog::load(&library)?;
        let (connection, screen, address) = match startup {
            Startup::Choose(None) if !catalog.entries.with(Vec::is_empty) => {
                (None, Screen::Connections, String::new())
            }
            Startup::Choose(address) => (None, Screen::Clients, address.unwrap_or_default()),
            Startup::Configure(client) => (
                None,
                Screen::Connection(client),
                client.profile().default_address.into(),
            ),
            Startup::Connected(connection) => {
                let address = connection.address.clone();
                (Some(connection), Screen::Workspace, address)
            }
        };
        Ok(Self {
            address: signal(address),
            request_headers: signal(String::new()),
            connection: signal(connection),
            connection_error: signal(String::new()),
            catalog,
            draft: ConnectionDraft::default(),
            screen: signal(screen),
            services: Services::from_owner(&owner)?,
            owner,
            sql: signal(String::new()),
            completion: Rc::new(Completion::new()?),
            execution: signal(Execution::Idle),
            results: signal(Results::default()),
            viewport: signal(Viewport::default()),
            library,
            browser: signal(Browser::default()),
            search: signal(String::new()),
            starred: signal(false),
            narrow_library: signal(false),
            menu: signal(None),
            menu_search: signal(String::new()),
            content_focus: RefCell::new(None),
            root: RefCell::new(None),
        })
    }
}

impl Terminal {
    fn idle(&self) -> bool {
        self.execution
            .with(|execution| matches!(execution, Execution::Idle))
    }

    fn running(&self) -> bool {
        self.execution
            .with(|execution| matches!(execution, Execution::Running(_)))
    }

    fn failed(&self) -> bool {
        self.execution
            .with(|execution| matches!(execution, Execution::Failed(_)))
    }

    fn can_run(&self) -> bool {
        self.connection.with(Option::is_some)
            && !self.running()
            && self.sql.with(|sql| !sql.trim().is_empty())
    }

    fn run(&self) -> Result<(), Error> {
        if !self.can_run() {
            return Ok(());
        }
        self.completion.dismiss();
        let sql = self.sql.get();
        self.save_query(Change::Executed)?;
        let (abort, registration) = AbortHandle::new_pair();
        let client = self.connection.with(|connection| {
            connection
                .as_ref()
                .expect("runnable queries have a connection")
                .client
                .clone()
        });
        let results = self.results.clone();
        let execution = self.execution.clone();
        let started = Instant::now();
        self.services.spawn(&self.owner, async move {
            let query = receive(client.as_ref(), sql, &results);
            execution.replace(match Abortable::new(query, registration).await {
                Ok(Ok(())) => Execution::Finished(format!(
                    "Complete · {:.2}s",
                    started.elapsed().as_secs_f64()
                )),
                Ok(Err(error)) => Execution::Failed(error.to_string()),
                Err(_) => Execution::Cancelled,
            });
        })?;
        self.results.replace(Results::default());
        self.viewport.update(Viewport::reset);
        self.execution.replace(Execution::Running(abort));
        Ok(())
    }

    fn cancel(&self) {
        self.execution.with_untracked(|execution| {
            if let Execution::Running(abort) = execution {
                abort.abort();
            }
        });
    }

    fn status(&self) -> String {
        self.execution.with(|execution| match execution {
            Execution::Idle => "Ready · Write SQL and choose Run".into(),
            Execution::Running(_) => "Streaming · Esc to cancel".into(),
            Execution::Finished(message) => message.clone(),
            Execution::Cancelled => "Cancelled · received rows are available".into(),
            Execution::Failed(error) => error.clone(),
        })
    }

    fn rows(&self) -> Vec<Row> {
        let viewport = self.viewport.get();
        self.results.with(|results| results.rows(viewport))
    }

    fn headers(&self) -> Vec<Cell> {
        let viewport = self.viewport.get();
        self.results.with(|results| results.headers(viewport))
    }

    fn position(&self) -> String {
        let viewport = self.viewport.get();
        self.results.with(|results| results.position(viewport))
    }

    fn empty_message(&self) -> &'static str {
        self.execution.with(|execution| match execution {
            Execution::Idle => "Your results will appear here. Run a query to get started.",
            Execution::Running(_) => "Waiting for the server…",
            Execution::Finished(_) => "Query completed with no rows.",
            Execution::Cancelled => "Query cancelled before any rows arrived.",
            Execution::Failed(_) => {
                "Query failed. Edit the SQL or check the connection, then retry."
            }
        })
    }
}

async fn receive(
    client: &dyn QueryClient,
    sql: String,
    results: &Signal<Results>,
) -> Result<(), sql_bomb::Error> {
    let mut query = client.query(sql).await?;
    results.update(|results| results.schema = query.schema);
    while let Some(batch) = query.batches.try_next().await? {
        results.update(|results| results.append(batch));
        tokio::task::yield_now().await;
    }
    Ok(())
}
