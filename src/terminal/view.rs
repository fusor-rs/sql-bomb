use super::connection::ConnectionPurpose;
use super::{Screen, Terminal, welcome::Welcome};
use crate::client::{CLIENTS, Client};
use crate::library::Collection;
use std::rc::Rc;

macro_rules! terminal_views {
    ($($name:ident $({ $input:ident: $type:ty })? => $template:literal),+ $(,)?) => {
        $(
            #[derive(fusor::FromInputs)]
            struct $name {
                #[input]
                terminal: Rc<Terminal>,
                $(
                    #[input]
                    $input: $type,
                )?
            }

            fusor::template!(backend = "hypercmd", $template);
        )+
    };
}

terminal_views! {
    SavedConnections => "ui/connections/saved.html",
    ConnectionForm { client: Client } => "ui/connections/form.html",
    ConnectionSecret { client: Client } => "ui/connections/secret.html",
    ConnectionCredentials { client: Client } => "ui/connections/credentials.html",
    RemoveConnection => "ui/connections/remove.html",
    ClientChooser => "ui/connections/clients.html",
    QueryWorkspace => "ui/workspace.html",
    QuerySuggestions => "ui/query/suggestions.html",
    QueryLibrary => "ui/library.html",
    ValueInspector => "ui/inspector.html",
    CommandMenu => "ui/menu.html",
}

fusor::template!(backend = "hypercmd", "ui/app.html");
