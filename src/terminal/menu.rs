use super::{Screen, Terminal};
use crate::library::Collection;
use hypercmd::{Error, Event, EventPayload, Input, Key, Node};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Action {
    Editor,
    History,
    Starred,
    Run,
    Star,
    Results,
    Inspect,
    Cancel,
    Connection,
    SaveConnection,
}

#[derive(PartialEq)]
pub(super) struct Command {
    pub(super) action: Action,
    pub(super) label: &'static str,
    pub(super) shortcut: &'static str,
    description: &'static str,
}

const COMMANDS: &[Command] = &[
    Command {
        action: Action::Editor,
        label: "Query editor",
        shortcut: "Ctrl+E",
        description: "Return to your SQL draft.",
    },
    Command {
        action: Action::History,
        label: "Query history",
        shortcut: "",
        description: "Search executed queries and preview their SQL before loading.",
    },
    Command {
        action: Action::Starred,
        label: "Starred queries",
        shortcut: "",
        description: "Browse the queries you have saved on this device.",
    },
    Command {
        action: Action::Run,
        label: "Run query",
        shortcut: "Ctrl+R",
        description: "Execute the SQL in the editor on the current connection.",
    },
    Command {
        action: Action::Star,
        label: "Star / unstar query",
        shortcut: "Ctrl+S",
        description: "Toggle the star on your draft or the selected library query.",
    },
    Command {
        action: Action::Results,
        label: "Browse results",
        shortcut: "Ctrl+L",
        description: "Return to the results grid; use arrows to select a cell.",
    },
    Command {
        action: Action::Inspect,
        label: "Inspect result value",
        shortcut: "",
        description: "Read the complete selected cell, including long values.",
    },
    Command {
        action: Action::Cancel,
        label: "Cancel query",
        shortcut: "",
        description: "Stop the running query and keep rows already received.",
    },
    Command {
        action: Action::Connection,
        label: "Change connection",
        shortcut: "",
        description: "Open a saved connection or set up a new one.",
    },
    Command {
        action: Action::SaveConnection,
        label: "Save connection…",
        shortcut: "",
        description: "Save the current connection for future sessions.",
    },
];

pub(super) struct Menu {
    origin: Node,
    pub(super) commands: Vec<&'static Command>,
    selected: usize,
}

impl Terminal {
    pub(super) fn remember_focus(&self, event: &Event) {
        if event.target.attribute("id").as_deref() != Some("query") {
            self.completion.dismiss();
        }
        self.content_focus.replace(Some(event.target.clone()));
    }

    pub(super) fn toggle_menu(&self, event: &Event) {
        self.completion.dismiss();
        if self.menu.with(Option::is_some) {
            self.close_menu();
            return;
        }
        self.menu_search.set(String::new());
        self.menu.replace(Some(Menu {
            origin: self
                .content_focus
                .borrow()
                .as_ref()
                .unwrap_or(&event.target)
                .clone(),
            commands: COMMANDS.iter().collect(),
            selected: 0,
        }));
        self.focus("menu-search");
    }

    pub(super) fn close_menu(&self) {
        if let Some(menu) = self.menu.replace(None) {
            menu.origin.request_focus();
        }
    }

    pub(super) fn filter_menu(&self) {
        let search = self
            .menu_search
            .with(|search| search.trim().trim_start_matches('/').to_ascii_lowercase());
        self.menu.update(|menu| {
            if let Some(menu) = menu {
                menu.commands = COMMANDS
                    .iter()
                    .filter(|command| command.label.to_ascii_lowercase().contains(&search))
                    .collect();
                menu.selected = 0;
            }
        });
    }

    pub(super) fn select_command(&self, action: Action) {
        self.menu.update(|menu| {
            let Some(menu) = menu else {
                return;
            };
            if let Some(index) = menu
                .commands
                .iter()
                .position(|command| command.action == action)
            {
                menu.selected = index;
            }
        });
    }

    pub(super) fn selected_command(&self) -> Option<&'static Command> {
        self.menu.with(|menu| {
            menu.as_ref()
                .and_then(|menu| menu.commands.get(menu.selected).copied())
        })
    }

    pub(super) fn menu_description(&self) -> &'static str {
        self.selected_command().map_or(
            "No matching actions. Try another word or clear the search.",
            |command| {
                self.unavailable(command.action)
                    .unwrap_or(command.description)
            },
        )
    }

    pub(super) fn unavailable(&self, action: Action) -> Option<&'static str> {
        if action != Action::Connection && self.connection.with(Option::is_none) {
            return Some("Choose a client and configure a connection first.");
        }
        match action {
            Action::Run if self.screen.get() != Screen::Workspace => {
                Some("Open Query editor to run its SQL.")
            }
            Action::Run | Action::Connection | Action::SaveConnection if self.running() => {
                Some("Cancel the running query or wait for it to finish.")
            }
            Action::Run if !self.can_run() => Some("Write SQL in the editor first."),
            Action::SaveConnection
                if self.connection.with(|connection| {
                    connection
                        .as_ref()
                        .is_some_and(|connection| connection.saved.is_some())
                }) =>
            {
                Some("This connection is already saved. Use Connection to edit it.")
            }
            Action::Star if self.screen.get() == Screen::Library => self
                .library_selection()
                .is_none()
                .then_some("Select a query first."),
            Action::Star if self.screen.get() != Screen::Workspace => {
                Some("Open Query editor or the library to star a query.")
            }
            Action::Star if self.sql.with(|sql| sql.trim().is_empty()) => {
                Some("Write SQL in the editor first.")
            }
            Action::Inspect if !self.can_inspect() => Some("Run a query that returns rows first."),
            Action::Cancel if !self.running() => Some("No query is running."),
            _ => None,
        }
    }

    pub(super) fn choose_command(&self, action: Action) -> Result<(), Error> {
        if self.unavailable(action).is_some() {
            return Ok(());
        }
        if matches!(action, Action::Run | Action::Star | Action::Cancel) {
            self.close_menu();
        } else {
            self.menu.replace(None);
        }
        match action {
            Action::Editor | Action::Results => {
                self.screen.set(Screen::Workspace);
                self.focus(if action == Action::Editor {
                    "query"
                } else {
                    "results"
                });
            }
            Action::History | Action::Starred => {
                let collection = if action == Action::History {
                    Collection::History
                } else {
                    Collection::Starred
                };
                self.search.set(String::new());
                self.collection(collection)?;
            }
            Action::Run => self.run()?,
            Action::Star => {
                self.toggle_star()?;
                if self.screen.get() == Screen::Library {
                    self.focus("library-search");
                }
            }
            Action::Inspect => self.inspect(),
            Action::Cancel => self.cancel(),
            Action::Connection => self.change_connection()?,
            Action::SaveConnection => self.save_current_connection(),
        }
        Ok(())
    }

    pub(super) fn menu_navigation(&self, event: &Event) -> Result<bool, Error> {
        let EventPayload::Input(Input::Key { key, modifiers, .. }) = event.payload else {
            return Ok(false);
        };
        match key {
            Key::Escape => self.close_menu(),
            Key::Char('k') if modifiers.control => self.close_menu(),
            Key::Enter if event.target.tag() == "input" => {
                if let Some(command) = self.selected_command() {
                    self.choose_command(command.action)?;
                }
            }
            Key::Up | Key::Down if !modifiers.control && !modifiers.alt && !modifiers.super_key => {
                self.move_command(if key == Key::Up { -1 } else { 1 });
            }
            _ => return Ok(false),
        }
        event.prevent_default();
        Ok(true)
    }

    fn move_command(&self, direction: isize) {
        let selected = self.menu.update(|menu| {
            let menu = menu
                .as_mut()
                .expect("menu navigation requires an open menu");
            menu.selected = menu
                .selected
                .saturating_add_signed(direction)
                .min(menu.commands.len().saturating_sub(1));
            menu.selected
        });
        if let Some(list) = self.find_control("menu-commands") {
            list.scroll_to(0, selected as u16);
        }
        self.focus("menu-search");
    }
}
