use super::{CONTROL, DRAFT, ENDPOINT, TerminalSession, library_directory, search_menu};
use crate::{
    client::{Client, Startup},
    library::{Collection, Credentials, Library, SavedConnection},
};
use hypercmd::{Input, Key, Modifiers};
use keyring::{Entry, mock::MockCredential};
use std::{cell::RefCell, collections::BTreeMap, fs, path::Path, rc::Rc};

const HEADERS: &str = "authorization: Bearer saved-connection-fixture";
type CredentialEntries = Rc<RefCell<BTreeMap<String, Rc<Entry>>>>;

fn connection_library(directory: &Path, entries: &CredentialEntries) -> Library {
    let mut library = Library::open(directory).unwrap();
    let entries = entries.clone();
    library.credentials = Box::new(move |account| {
        Ok(entries
            .borrow_mut()
            .entry(account.into())
            .or_insert_with(|| {
                Rc::new(Entry::new_with_credential(Box::<MockCredential>::default()))
            })
            .clone())
    });
    library
}

fn fill_field(session: &mut TerminalSession, id: &str, value: &str) {
    let field = session.scope.root().find(id).unwrap();
    session.controls.set_focus(&field).unwrap();
    session.draw();
    session.key(Key::Char('a'), CONTROL);
    session.input(Input::Paste(value.into()));
}

#[test]
fn saved_connections_reopen_with_credentials_and_support_editing_and_removal() {
    let directory = library_directory("saved-connections");
    let entries = CredentialEntries::default();
    let mut session = TerminalSession::start(
        connection_library(&directory, &entries),
        Startup::Choose(None),
    );
    create_session_only_connection(&mut session);
    rename_session_only_connection(&mut session);
    drop(session);
    let library = connection_library(&directory, &entries);
    assert_eq!(
        library.connections("").unwrap()[0].credentials,
        Credentials::Session
    );
    let mut session = TerminalSession::start(library, Startup::Choose(None));
    profile_destination_is_visible(&mut session);
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("connection-search")
    );
    session.key(Key::Enter, Modifiers::default());
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("request-headers")
    );
    session.click("connect");
    assert_eq!(
        session
            .scope
            .root()
            .find("connection-error")
            .unwrap()
            .text(),
        "Enter the request headers for this connection."
    );
    fill_field(&mut session, "request-headers", HEADERS);
    session.click("remember-credentials");
    session.click("connect");
    drop(session);
    let mut session = TerminalSession::start(
        connection_library(&directory, &entries),
        Startup::Choose(None),
    );
    session.click("open-connection");
    assert_eq!(session.controls.focus(), session.scope.root().find("query"));
    session.input(Input::Paste(DRAFT.into()));
    session.key(Key::Char('s'), CONTROL);
    edit_saved_connection(&mut session, &directory, &entries);
    remove_saved_connection(&mut session, &directory, &entries);
    save_current_workspace(&mut session, &directory, &entries);
    drop(session);
    fs::remove_dir_all(directory).unwrap();
}

fn profile_destination_is_visible(session: &mut TerminalSession) {
    let frame = session.draw();
    let visible: String = frame
        .buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(visible.contains("Flight SQL · http://localhost:50051"));
}

fn create_session_only_connection(session: &mut TerminalSession) {
    session.key(Key::Enter, Modifiers::default());
    let root = session.scope.root();
    assert!(!root.find("save-connection").unwrap().checked());
    assert_eq!(root.find("connection-name"), None);
    fill_field(session, "request-headers", HEADERS);
    session.click("save-connection");
    assert_eq!(root.find("connection-name").unwrap().value(), "localhost");
    assert!(!root.find("remember-credentials").unwrap().checked());
    connection_actions_stay_visible(session);
    fill_field(session, "connection-name", "Local analytics");
    session.click("connect");
    assert_eq!(
        root.find("connection-label").unwrap().text(),
        "Local analytics · Flight SQL · http://localhost:50051"
    );
}

fn connection_actions_stay_visible(session: &mut TerminalSession) {
    for size in [super::MENU_SIZE, (40, 20), super::WIDE_SIZE] {
        session.size = size;
        let frame = session.draw();
        let submit = session.scope.root().find("connect").unwrap();
        let entry = frame
            .entries
            .iter()
            .find(|entry| entry.node == submit)
            .unwrap();
        assert_eq!(entry.content.intersection(entry.clip).height, 1);
    }
}

fn rename_session_only_connection(session: &mut TerminalSession) {
    session.click("change-connection");
    session.click("edit-connection");
    assert_eq!(
        session
            .scope
            .root()
            .find("request-headers")
            .unwrap()
            .value(),
        ""
    );
    fill_field(session, "connection-name", "Local analytics");
    session.click("connect");
}

fn edit_saved_connection(
    session: &mut TerminalSession,
    directory: &Path,
    entries: &CredentialEntries,
) {
    session.click("change-connection");
    session.click("edit-connection");
    fill_field(session, "connection-name", "Discarded edit");
    session.key(Key::Escape, Modifiers::default());
    assert_eq!(
        connection_library(directory, entries)
            .connections("")
            .unwrap()[0]
            .name,
        "Local analytics"
    );
    session.click("edit-connection");
    assert_eq!(
        session
            .scope
            .root()
            .find("request-headers")
            .unwrap()
            .value(),
        HEADERS
    );
    reject_empty_remembered_headers(session);
    fill_field(session, "connection-name", "Renamed analytics");
    assert_eq!(
        session.scope.root().find("connect").unwrap().text(),
        "Save changes"
    );
    session.click("connect");
    fill_field(session, "connection-search", "missing connection");
    assert_eq!(
        session
            .scope
            .root()
            .find("open-connection")
            .unwrap()
            .attribute("disabled"),
        Some(String::new())
    );
    session.click("new-connection");
    session.key(Key::Escape, Modifiers::default());
    fill_field(session, "connection-search", "renamed");
    session.key(Key::Enter, Modifiers::default());
    assert_eq!(
        session
            .scope
            .root()
            .find("connection-label")
            .unwrap()
            .text(),
        "Renamed analytics · Flight SQL · http://localhost:50051"
    );
    assert_eq!(session.scope.root().find("query").unwrap().value(), DRAFT);
}

fn reject_empty_remembered_headers(session: &mut TerminalSession) {
    fill_field(session, "request-headers", "");
    session.click("connect");
    assert_eq!(
        session
            .scope
            .root()
            .find("connection-error")
            .unwrap()
            .text(),
        "Enter headers to remember, or uncheck Remember headers."
    );
    fill_field(session, "request-headers", HEADERS);
}

fn remove_saved_connection(
    session: &mut TerminalSession,
    directory: &Path,
    entries: &CredentialEntries,
) {
    session.click("change-connection");
    session.click("remove-connection");
    assert_eq!(
        session.controls.focus(),
        session.scope.root().find("keep-connection")
    );
    session.key(Key::Enter, Modifiers::default());
    assert_eq!(
        connection_library(directory, entries)
            .connections("")
            .unwrap()
            .len(),
        1
    );
    session.click("remove-connection");
    session.click("confirm-remove-connection");
    let library = connection_library(directory, entries);
    assert_eq!(library.connections("").unwrap().len(), 0);
    assert_eq!(library.count(Collection::Starred, DRAFT).unwrap(), 1);
    session.key(Key::Escape, Modifiers::default());
    assert_eq!(session.scope.root().find("query").unwrap().value(), DRAFT);
    assert_eq!(
        session
            .scope
            .root()
            .find("connection-label")
            .unwrap()
            .text(),
        "Flight SQL · http://localhost:50051"
    );
}

fn save_current_workspace(
    session: &mut TerminalSession,
    directory: &Path,
    entries: &CredentialEntries,
) {
    search_menu(session, "save connection");
    session.key(Key::Enter, Modifiers::default());
    fill_field(session, "address", "http://different-server:50051");
    assert_eq!(
        session.scope.root().find("address").unwrap().value(),
        ENDPOINT
    );
    fill_field(session, "connection-name", "Saved from workspace");
    assert_eq!(
        session.scope.root().find("connect").unwrap().text(),
        "Save connection"
    );
    session.click("connect");
    assert_eq!(session.controls.focus(), session.scope.root().find("query"));
    assert_eq!(session.scope.root().find("query").unwrap().value(), DRAFT);
    let library = connection_library(directory, entries);
    assert_eq!(
        library.connections("").unwrap()[0].name,
        "Saved from workspace"
    );
    assert_eq!(library.count(Collection::History, "").unwrap(), 0);
}

#[test]
fn profiles_keep_secrets_out_of_sqlite_and_preserve_credentials_when_saving_fails() {
    let directory = library_directory("connection-storage");
    let entries = CredentialEntries::default();
    let library = connection_library(&directory, &entries);
    let connection = Client::FlightSql
        .connect(ENDPOINT.into(), HEADERS.into())
        .unwrap();
    let profile = library
        .save_connection(None, "Original", &connection, Credentials::Keyring)
        .unwrap();
    assert_eq!(
        library.connection_headers(&profile).unwrap(),
        Some(HEADERS.into())
    );
    let second = library
        .save_connection(None, "Another", &connection, Credentials::Session)
        .unwrap();
    library.opened_connection(&second.id).unwrap();
    library.opened_connection(&profile.id).unwrap();
    assert_eq!(
        library
            .connections("")
            .unwrap()
            .iter()
            .map(|profile| profile.name.as_str())
            .collect::<Vec<_>>(),
        ["Original", "Another"]
    );
    reject_secret_bearing_addresses();
    credential_failure_keeps_profile(&library, &profile, &entries);
    locked_database_restores_credentials(&library, &profile, &directory);
    let reopened = connection_library(&directory, &entries);
    let saved = reopened.connections("original").unwrap();
    assert_eq!(saved[0].address, ENDPOINT);
    assert_eq!(
        reopened.connection_headers(&saved[0]).unwrap(),
        Some(HEADERS.into())
    );
    for file in fs::read_dir(&directory).unwrap() {
        let bytes = fs::read(file.unwrap().path()).unwrap();
        assert!(
            !bytes
                .windows(HEADERS.len())
                .any(|window| window == HEADERS.as_bytes())
        );
    }
    reopened.remove_connection(&saved[0]).unwrap();
    assert_eq!(reopened.connection_headers(&saved[0]).unwrap(), None);
    assert_eq!(reopened.connections("").unwrap().len(), 1);
    drop((library, reopened));
    fs::remove_dir_all(directory).unwrap();
}

fn credential_failure_keeps_profile(
    library: &Library,
    profile: &SavedConnection,
    entries: &CredentialEntries,
) {
    let entry = entries.borrow().get(&profile.id.0).unwrap().clone();
    entry
        .get_credential()
        .downcast_ref::<MockCredential>()
        .unwrap()
        .set_error(keyring::Error::NoStorageAccess(Box::new(
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        )));
    let connection = Client::FlightSql
        .connect(ENDPOINT.into(), "authorization: Bearer replacement".into())
        .unwrap();
    let error = library
        .save_connection(
            Some(&profile.id),
            "Should not save",
            &connection,
            Credentials::Keyring,
        )
        .err()
        .unwrap();
    assert_eq!(
        error.to_string(),
        "Cannot access saved credentials; unlock your OS credential store or use session-only headers"
    );
    assert_eq!(library.connections("original").unwrap()[0].name, "Original");
    assert_eq!(
        library.connection_headers(profile).unwrap(),
        Some(HEADERS.into())
    );
}

fn locked_database_restores_credentials(
    library: &Library,
    profile: &SavedConnection,
    directory: &Path,
) {
    let path = fs::read_dir(directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let reader = rusqlite::Connection::open(path).unwrap();
    reader
        .execute_batch("BEGIN; SELECT * FROM saved_connections")
        .unwrap();
    let connection = Client::FlightSql
        .connect(ENDPOINT.into(), "authorization: Bearer replacement".into())
        .unwrap();
    let error = library
        .save_connection(
            Some(&profile.id),
            "Uncommitted",
            &connection,
            Credentials::Keyring,
        )
        .err()
        .unwrap();
    assert_eq!(
        error.to_string(),
        "Cannot read or save the query library; check its database and file permissions: database is locked"
    );
    reader.execute_batch("ROLLBACK").unwrap();
    assert_eq!(library.connections("original").unwrap()[0].name, "Original");
    assert_eq!(
        library.connection_headers(profile).unwrap(),
        Some(HEADERS.into())
    );
}

fn reject_secret_bearing_addresses() {
    for address in [
        "http://user:password@localhost:50051",
        "http://localhost:50051?token=secret",
        "http://localhost:50051/secret",
    ] {
        let error = Client::FlightSql
            .connect(address.into(), String::new())
            .err()
            .unwrap();
        assert_eq!(
            error.to_string(),
            "Use a server URL without a path, query, or credentials; put credentials in request headers"
        );
    }
}
