use super::connection::ConnectionPurpose;
use super::{Screen, Terminal, welcome::Welcome};
use crate::client::{CLIENTS, Client};
use crate::library::Collection;
use std::rc::Rc;

macro_rules! terminal_views {
    ($($name:ident => $template:literal),+ $(,)?) => {
        $(
            #[derive(fusor::FromInputs)]
            struct $name {
                #[input]
                terminal: Rc<Terminal>,
            }

            fusor::template!(backend = "hypercmd", $template);
        )+
    };
}

terminal_views! {
    SavedConnections => "ui/connections/saved.html",
    ConnectionHeaders => "ui/connections/headers.html",
    ConnectionCredentials => "ui/connections/credentials.html",
    RemoveConnection => "ui/connections/remove.html",
    ClientChooser => "ui/connections/clients.html",
    QueryWorkspace => "ui/workspace.html",
    QuerySuggestions => "ui/query/suggestions.html",
    QueryLibrary => "ui/library.html",
    ValueInspector => "ui/inspector.html",
    CommandMenu => "ui/menu.html",
}

#[derive(fusor::FromInputs)]
struct ConnectionForm {
    #[input]
    terminal: Rc<Terminal>,
    #[input]
    client: Client,
}

fusor::template!(backend = "hypercmd", "ui/connections/form.html");
fusor::template!(backend = "hypercmd", "ui/app.html");
