mod connections;

use crate::{
    client::{Client, Startup},
    library::{Change, Collection, Library},
    terminal::{Screen, Terminal},
};
use hypercmd::{Controller, Input, Key, KeyKind, Modifiers, Scope, layout::Presentation};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

const ENDPOINT: &str = "http://localhost:50051";
const OTHER_ENDPOINT: &str = "http://localhost:50052";
const SAVED_SQL: &str = "-- café\nSELECT '東京_100%';";
const DRAFT: &str = "SELECT 'unfinished draft';";
const CONTROL: Modifiers = Modifiers {
    control: true,
    shift: false,
    alt: false,
    super_key: false,
};
const WIDE_SIZE: (u16, u16) = (140, 40);
const NARROW_SIZE: (u16, u16) = (80, 30);
const MENU_SIZE: (u16, u16) = (80, 24);
const MAX_LAYOUT_PASSES: usize = 10;

fn library_directory(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sql-bomb-{name}-{}", std::process::id()))
}

#[test]
fn history_and_stars_persist_across_sessions_without_losing_executions() {
    let directory = library_directory("persistence");
    let first = Library::open(&directory).unwrap();
    let second = Library::open(&directory).unwrap();
    for (library, endpoint, sql) in [
        (&first, ENDPOINT, SAVED_SQL),
        (&second, ENDPOINT, "SELECT 2;"),
        (&second, OTHER_ENDPOINT, SAVED_SQL),
        (&first, ENDPOINT, SAVED_SQL),
    ] {
        library.save(endpoint, sql, Change::Executed).unwrap();
    }
    first.save(ENDPOINT, SAVED_SQL, Change::ToggleStar).unwrap();
    assert!(second.starred(ENDPOINT, SAVED_SQL).unwrap());
    assert!(!second.starred(OTHER_ENDPOINT, SAVED_SQL).unwrap());
    drop((first, second));
    let reopened = Library::open(&directory).unwrap();
    assert_eq!(reopened.count(Collection::History, "").unwrap(), 4);
    assert_eq!(reopened.count(Collection::Starred, "").unwrap(), 1);
    assert_eq!(reopened.count(Collection::History, "東京_100%").unwrap(), 3);
    assert_eq!(
        reopened.count(Collection::History, OTHER_ENDPOINT).unwrap(),
        1
    );
    let page = reopened.entries(Collection::History, "", 1..3).unwrap();
    assert_eq!(page[0].sql, SAVED_SQL);
    assert_eq!(page[0].endpoint, OTHER_ENDPOINT);
    assert_eq!(page[1].sql, "SELECT 2;");
    assert_eq!(
        reopened
            .entries(Collection::History, "", 4..5)
            .unwrap()
            .len(),
        0
    );
    unstar_preserves_history(&reopened);
    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for file in fs::read_dir(&directory).unwrap() {
        assert_eq!(
            file.unwrap().metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    drop(reopened);
    fs::remove_dir_all(directory).unwrap();
}

fn unstar_preserves_history(library: &Library) {
    library
        .save(ENDPOINT, SAVED_SQL, Change::ToggleStar)
        .unwrap();
    assert_eq!(library.count(Collection::Starred, "").unwrap(), 0);
    assert_eq!(library.count(Collection::History, "").unwrap(), 4);
    library.save(ENDPOINT, DRAFT, Change::ToggleStar).unwrap();
    let stars = library.entries(Collection::Starred, "", 0..1).unwrap();
    assert_eq!(stars[0].sql, DRAFT);
    assert_eq!(library.count(Collection::History, "").unwrap(), 4);
}

struct TerminalSession {
    scope: Scope,
    controls: Controller,
    size: (u16, u16),
}

impl TerminalSession {
    fn new(library: Library, screen: &Screen) -> Self {
        let startup = match screen {
            Screen::Workspace => Startup::Connected(
                Client::FlightSql
                    .connect(ENDPOINT.into(), String::new())
                    .unwrap(),
            ),
            Screen::Connection(client) => Startup::Configure(*client),
            _ => panic!("Session fixtures start in the workspace or connection form"),
        };
        Self::start(library, startup)
    }

    fn start(library: Library, startup: Startup) -> Self {
        let scope = hypercmd::mount::<Terminal>((startup, library)).unwrap();
        scope.publish();
        let controls = Controller::new(scope.root());
        let mut session = Self {
            scope,
            controls,
            size: WIDE_SIZE,
        };
        session.draw();
        session
    }

    fn draw(&mut self) -> Presentation {
        for _ in 0..MAX_LAYOUT_PASSES {
            self.scope.take_dirty();
            let focus = self.controls.focus();
            let frame = self.frame();
            self.controls.presented(frame).unwrap();
            if !self.scope.take_dirty() && self.controls.focus() == focus {
                return self.frame();
            }
        }
        panic!("terminal layout did not settle");
    }

    fn frame(&mut self) -> Presentation {
        let focus = self.controls.focus();
        let mut frame = hypercmd::layout::render(
            &self.scope.root(),
            self.size,
            focus.as_ref(),
            self.controls.scrolls_mut(),
            &Default::default(),
        )
        .unwrap();
        self.controls.decorate(&mut frame);
        frame
    }

    fn input(&mut self, input: Input) {
        self.controls.handle(input).unwrap();
        self.draw();
        assert_eq!(self.scope.take_errors(), Vec::new());
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.input(Input::Key {
            key,
            modifiers,
            kind: KeyKind::Press,
        });
    }

    fn click(&mut self, id: &str) {
        let node = self.scope.root().find(id).unwrap();
        let frame = self.draw();
        let entry = frame
            .entries
            .iter()
            .find(|entry| entry.node == node)
            .unwrap();
        self.input(Input::Click {
            column: entry.content.x,
            row: entry.content.y,
        });
    }
}

#[test]
fn startup_accepts_input_immediately_and_replaces_branding_when_a_query_runs() {
    let directory = library_directory("startup");
    for (screen, destination) in [
        (Screen::Workspace, "query"),
        (Screen::Connection(Client::FlightSql), "address"),
    ] {
        let library = Library::open(&directory).unwrap();
        let mut session = TerminalSession::new(library, &screen);
        session.size = MENU_SIZE;
        session.draw();
        let root = session.scope.root();
        assert_eq!(session.controls.focus(), root.find(destination));
        assert_eq!(root.find("skip-splash"), None);
        if destination == "query" {
            assert_eq!(root.find("welcome-title").unwrap().text(), "sql-bomb");
            session.input(Input::Paste(DRAFT.into()));
            assert_eq!(root.find("query").unwrap().value(), DRAFT);
            logo_survives_resizing(&mut session);
            session.key(Key::Char('s'), CONTROL);
            assert_eq!(root.find("star-query").unwrap().text(), "★ Starred  ^S");
            session.key(Key::Char('r'), CONTROL);
            assert_eq!(root.find("welcome"), None);
            assert_eq!(session.controls.focus(), root.find("query"));
        } else {
            assert_eq!(root.find("welcome"), None);
            session.input(Input::Paste("x".into()));
            assert_eq!(
                root.find("address").unwrap().value(),
                "xhttp://localhost:50051"
            );
        }
    }
    let library = Library::open(&directory).unwrap();
    assert_eq!(library.count(Collection::History, DRAFT).unwrap(), 1);
    assert_eq!(library.count(Collection::Starred, DRAFT).unwrap(), 1);
    drop(library);
    fs::remove_dir_all(directory).unwrap();
}

fn logo_survives_resizing(session: &mut TerminalSession) {
    for size in [WIDE_SIZE, MENU_SIZE, (40, 20), (80, 18)] {
        session.size = size;
        let frame = session.draw();
        assert_eq!(
            frame
                .entries
                .iter()
                .filter(|entry| entry.node.attribute("class").as_deref() == Some("welcome-art"))
                .count(),
            1
        );
    }
}

#[test]
fn client_selection_and_configuration_precede_the_workspace() {
    let directory = library_directory("clients");
    for (arguments, initial, address) in [
        (vec![], "client-cards", ENDPOINT),
        (vec![OTHER_ENDPOINT], "client-cards", OTHER_ENDPOINT),
        (vec!["--flightsql"], "address", ENDPOINT),
        (vec!["--flightsql", ENDPOINT], "query", ENDPOINT),
    ] {
        let crate::cli::Command::Start(startup) =
            crate::cli::parse(arguments.iter().map(|value| (*value).to_owned())).unwrap()
        else {
            panic!("Connection arguments start the application");
        };
        let mut session = TerminalSession::start(Library::open(&directory).unwrap(), startup);
        session.size = MENU_SIZE;
        session.draw();
        let root = session.scope.root();
        if initial == "client-cards" {
            choose_client_card(&mut session, &arguments);
        } else {
            assert_eq!(session.controls.focus(), root.find(initial));
        }
        if initial != "query" {
            assert_eq!(session.controls.focus(), root.find("address"));
            assert_eq!(root.find("address").unwrap().value(), address);
            if arguments.is_empty() {
                rejects_invalid_connection_settings(&mut session);
            }
            session.click("connect");
        }
        assert_eq!(session.controls.focus(), root.find("query"));
        assert_eq!(
            root.find("connection-label").unwrap().text(),
            format!("Flight SQL · {address}")
        );
        session.size = MENU_SIZE;
        session.draw();
        session.click("change-connection");
        session.key(Key::Escape, Modifiers::default());
        assert_eq!(session.controls.focus(), root.find("query"));
    }
    assert_eq!(
        Library::open(&directory)
            .unwrap()
            .count(Collection::History, "")
            .unwrap(),
        0
    );
    fs::remove_dir_all(directory).unwrap();
}

fn choose_client_card(session: &mut TerminalSession, arguments: &[&str]) {
    let root = session.scope.root();
    let cards = root.find("client-cards").unwrap();
    let buttons: Vec<_> = cards
        .descendants()
        .filter(|node| node.tag() == "button")
        .collect();
    assert_eq!(buttons.len(), 1);
    assert_eq!(session.controls.focus().as_ref(), buttons.first());
    assert_eq!(
        root.find("connection-label").unwrap().text(),
        "Choose a client to get started"
    );
    session.key(Key::Char('e'), CONTROL);
    assert_eq!(session.controls.focus().as_ref(), buttons.first());
    session.click("open-menu");
    session.key(Key::Escape, Modifiers::default());
    assert_eq!(session.controls.focus().as_ref(), buttons.first());
    if arguments.is_empty() {
        session.key(Key::Enter, Modifiers::default());
    } else {
        let frame = session.draw();
        let card = frame
            .entries
            .iter()
            .find(|entry| entry.node == buttons[0])
            .unwrap();
        session.input(Input::Click {
            column: card.content.x,
            row: card.content.y,
        });
    }
}

fn rejects_invalid_connection_settings(session: &mut TerminalSession) {
    let root = session.scope.root();
    session.key(Key::Char('a'), CONTROL);
    session.input(Input::Paste("localhost:50051".into()));
    session.click("connect");
    assert_eq!(
        root.find("connection-error").unwrap().text(),
        "Invalid connection address; use http://host:port or https://host:port"
    );
    session.click("address");
    session.key(Key::Char('a'), CONTROL);
    session.input(Input::Paste(ENDPOINT.into()));
    session.click("request-headers");
    session.input(Input::Paste("not-a-header".into()));
    session.click("connect");
    assert_eq!(
        root.find("connection-error").unwrap().text(),
        "Invalid request headers; use one ASCII name: value per line (no binary headers)"
    );
    session.click("request-headers");
    session.key(Key::Char('a'), CONTROL);
    session.input(Input::Paste("authorization: Bearer example".into()));
    session.key(Key::Enter, Modifiers::default());
    session.input(Input::Paste("x-tenant: example".into()));
    assert_eq!(
        root.find("request-headers").unwrap().value(),
        "authorization: Bearer example\nx-tenant: example"
    );
    session.key(Key::Escape, Modifiers::default());
    session.key(Key::Enter, Modifiers::default());
    assert_eq!(session.controls.focus(), root.find("address"));
    assert_eq!(
        root.find("request-headers").unwrap().value(),
        "authorization: Bearer example\nx-tenant: example"
    );
    session.size = (40, 20);
    session.draw();
    session.key(Key::Tab, Modifiers::default());
    session.key(Key::Tab, Modifiers::default());
    assert_eq!(session.controls.focus(), root.find("save-connection"));
    session.key(Key::Tab, Modifiers::default());
    let connect = root.find("connect").unwrap();
    assert_eq!(session.controls.focus().as_ref(), Some(&connect));
    let frame = session.draw();
    let entry = frame
        .entries
        .iter()
        .find(|entry| entry.node == connect)
        .unwrap();
    assert_eq!(entry.content.intersection(entry.clip).height, 1);
}

#[test]
fn library_search_preview_and_recall_preserve_the_draft_until_loaded() {
    let directory = library_directory("interaction");
    let library = Library::open(&directory).unwrap();
    for number in 0..25 {
        library
            .save(ENDPOINT, &format!("SELECT {number};"), Change::Executed)
            .unwrap();
    }
    library
        .save(OTHER_ENDPOINT, SAVED_SQL, Change::Executed)
        .unwrap();
    let mut session = TerminalSession::new(library, &Screen::Workspace);
    session.input(Input::Paste(DRAFT.into()));
    session.key(Key::Char('s'), CONTROL);
    assert_eq!(
        session.scope.root().find("star-query").unwrap().text(),
        "★ Starred  ^S"
    );
    session.key(Key::Char('p'), CONTROL);
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("library-search")
    );
    assert_eq!(
        session.scope.root().find("library-preview").unwrap().text(),
        SAVED_SQL
    );
    search_and_cancel(&mut session, &directory);
    browse_and_load(&mut session);
    session.key(Key::Char('r'), CONTROL);
    session.key(Key::Escape, Modifiers::default());
    let persisted = Library::open(&directory).unwrap();
    assert_eq!(persisted.count(Collection::History, SAVED_SQL).unwrap(), 2);
    assert_eq!(persisted.count(Collection::Starred, "").unwrap(), 1);
    drop((session, persisted));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn menu_discovers_actions_without_changing_sql_or_running_recalled_queries() {
    let directory = library_directory("menu");
    let library = Library::open(&directory).unwrap();
    library.save(ENDPOINT, SAVED_SQL, Change::Executed).unwrap();
    let mut session = TerminalSession::new(library, &Screen::Workspace);
    session.size = MENU_SIZE;
    session.draw();
    menu_preserves_editor(&mut session);
    menu_browses_queries(&mut session);
    menu_explains_unavailable_actions(&mut session);
    let persisted = Library::open(&directory).unwrap();
    assert_eq!(persisted.count(Collection::History, "").unwrap(), 1);
    assert_eq!(persisted.count(Collection::Starred, "").unwrap(), 1);
    drop((session, persisted));
    fs::remove_dir_all(directory).unwrap();
}

fn menu_preserves_editor(session: &mut TerminalSession) {
    session.input(Input::Paste("SELECT 8 / 2;".into()));
    session.key(Key::Home, Modifiers::default());
    session.key(Key::Char('/'), Modifiers::default());
    assert_eq!(
        session.scope.root().find("query").unwrap().value(),
        "/SELECT 8 / 2;"
    );
    session.click("open-menu");
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("menu-search")
    );
    session.key(Key::Escape, Modifiers::default());
    assert_eq!(session.controls.focus(), session.scope.root().find("query"));
    session.key(Key::Char('*'), Modifiers::default());
    assert_eq!(
        session.scope.root().find("query").unwrap().value(),
        "/*SELECT 8 / 2;"
    );
    session.key(Key::Char('a'), CONTROL);
    session.key(Key::Char('k'), CONTROL);
    session.click("close-menu");
    session.input(Input::Paste(DRAFT.into()));
    assert_eq!(session.scope.root().find("query").unwrap().value(), DRAFT);
    session.key(Key::Char('l'), CONTROL);
    session.key(Key::Char('/'), Modifiers::default());
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("menu-search")
    );
    session.key(Key::Escape, Modifiers::default());
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("results")
    );
}

fn search_menu(session: &mut TerminalSession, search: &str) {
    session.key(Key::Char('k'), CONTROL);
    session.input(Input::Paste(search.into()));
}

fn menu_browses_queries(session: &mut TerminalSession) {
    search_menu(session, "/HISTORY");
    session.key(Key::Enter, Modifiers::default());
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("library-search")
    );
    assert_eq!(
        session.scope.root().find("library-preview").unwrap().text(),
        SAVED_SQL
    );
    session.key(Key::Char('/'), Modifiers::default());
    assert_eq!(
        session.scope.root().find("library-search").unwrap().value(),
        "/"
    );
    session.key(Key::Backspace, Modifiers::default());
    search_menu(session, "unstar");
    session.key(Key::Enter, Modifiers::default());
    assert_eq!(
        session.scope.root().find("star-entry").unwrap().text(),
        "★ Unstar  ^S"
    );
    search_menu(session, "starred");
    session.key(Key::Enter, Modifiers::default());
    assert_eq!(
        session.scope.root().find("library-preview").unwrap().text(),
        SAVED_SQL
    );
    menu_unstars_focused_entry(session);
    session.click("open-menu");
    session.click("open-library");
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("library-search")
    );
    session.click("open-menu");
    session.key(Key::Down, Modifiers::default());
    session.key(Key::Up, Modifiers::default());
    session.key(Key::Enter, Modifiers::default());
    assert_eq!(session.controls.focus(), session.scope.root().find("query"));
    assert_eq!(session.scope.root().find("query").unwrap().value(), DRAFT);
}

fn menu_unstars_focused_entry(session: &mut TerminalSession) {
    let entry = session
        .scope
        .root()
        .find("library-list")
        .unwrap()
        .descendants()
        .find(|node| node.tag() == "button")
        .unwrap();
    session.controls.set_focus(&entry).unwrap();
    session.draw();
    search_menu(session, "unstar");
    session.key(Key::Enter, Modifiers::default());
    session.key(Key::Char('k'), CONTROL);
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("menu-search")
    );
    session.input(Input::Paste("history".into()));
    session.key(Key::Enter, Modifiers::default());
    search_menu(session, "unstar");
    session.key(Key::Enter, Modifiers::default());
    assert_eq!(
        session.scope.root().find("star-entry").unwrap().text(),
        "★ Unstar  ^S"
    );
}

fn menu_explains_unavailable_actions(session: &mut TerminalSession) {
    search_menu(session, "cancel");
    assert_eq!(
        session
            .scope
            .root()
            .find("menu-description")
            .unwrap()
            .text(),
        "No query is running."
    );
    session.key(Key::Enter, Modifiers::default());
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("menu-search")
    );
    session.key(Key::Char('a'), CONTROL);
    session.input(Input::Paste("no-such-action".into()));
    assert_eq!(
        session
            .scope
            .root()
            .find("menu-description")
            .unwrap()
            .text(),
        "No matching actions. Try another word or clear the search."
    );
    session.key(Key::Enter, Modifiers::default());
    session.key(Key::Char('e'), CONTROL);
    assert_eq!(session.controls.focus(), session.scope.root().find("query"));
    assert_eq!(session.scope.root().find("query").unwrap().value(), DRAFT);
}

fn search_and_cancel(session: &mut TerminalSession, directory: &Path) {
    pointer_navigation(session);
    session.key(Key::Down, Modifiers::default());
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("library-search")
    );
    assert_eq!(
        session.scope.root().find("library-preview").unwrap().text(),
        "SELECT 24;"
    );
    session.key(Key::PageDown, Modifiers::default());
    assert_eq!(
        session.scope.root().find("library-preview").unwrap().text(),
        "SELECT 16;"
    );
    locked_history_preserves_selection(session, directory);
    session.input(Input::Paste("東京".into()));
    assert_eq!(
        session.scope.root().find("library-preview").unwrap().text(),
        SAVED_SQL
    );
    session.key(Key::Escape, Modifiers::default());
    assert_eq!(session.controls.focus(), session.scope.root().find("query"));
    assert_eq!(session.scope.root().find("query").unwrap().value(), DRAFT);
}

fn locked_history_preserves_selection(session: &mut TerminalSession, directory: &Path) {
    let path = fs::read_dir(directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let writer = rusqlite::Connection::open(path).unwrap();
    writer.execute_batch("BEGIN EXCLUSIVE").unwrap();
    session
        .controls
        .handle(Input::Key {
            key: Key::PageUp,
            modifiers: Modifiers::default(),
            kind: KeyKind::Press,
        })
        .unwrap();
    assert_eq!(
        session.scope.take_errors(),
        vec![hypercmd::Error::new(
            hypercmd::ErrorKind::Service,
            concat!(
                "Cannot read or save the query library; ",
                "check its database and file permissions: database is locked"
            )
        )]
    );
    session.draw();
    assert_eq!(
        session.scope.root().find("library-preview").unwrap().text(),
        "SELECT 16;"
    );
    writer.execute_batch("ROLLBACK").unwrap();
}

fn pointer_navigation(session: &mut TerminalSession) {
    let frame = session.draw();
    let list = session.scope.root().find("library-list").unwrap();
    let entry = frame
        .entries
        .iter()
        .find(|entry| entry.node == list)
        .unwrap();
    session.input(Input::Scroll {
        column: entry.content.x,
        row: entry.content.y,
        rows: 2,
        columns: 0,
    });
    assert_eq!(
        session.scope.root().find("library-preview").unwrap().text(),
        "SELECT 23;"
    );
    session.key(Key::Up, Modifiers::default());
    session.key(Key::Up, Modifiers::default());
}

fn browse_and_load(session: &mut TerminalSession) {
    session.click("open-library");
    session.key(Key::Char('a'), CONTROL);
    session.key(Key::Backspace, Modifiers::default());
    session.click("starred-tab");
    assert_eq!(
        session.scope.root().find("library-preview").unwrap().text(),
        DRAFT
    );
    session.key(Key::Char('s'), CONTROL);
    assert_eq!(session.scope.root().find("library-preview"), None);
    session.click("history-tab");
    session.size = NARROW_SIZE;
    let frame = session.draw();
    let root = session.scope.root();
    let list = frame
        .entries
        .iter()
        .find(|entry| Some(&entry.node) == root.find("library-list").as_ref())
        .unwrap();
    let preview = frame
        .entries
        .iter()
        .find(|entry| Some(&entry.node) == root.find("library-preview").as_ref())
        .unwrap();
    assert!(preview.content.y > list.content.y + list.content.height);
    session.input(Input::Paste("東京".into()));
    session.click("star-entry");
    session.click("load-query");
    assert_eq!(
        session.scope.root().find("query").unwrap().value(),
        SAVED_SQL
    );
    assert_eq!(session.controls.focus(), session.scope.root().find("query"));
    assert_eq!(
        session.scope.root().find("star-query").unwrap().text(),
        "☆ Star  ^S"
    );
}
