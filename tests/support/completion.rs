#[path = "terminal.rs"]
mod terminal;

use std::fs;
use terminal::Terminal;

const TABLE_DRAFT: &str = "SELECT * FROM warehouse.travel.fl WHERE true;";
const TABLE_SUFFIX: &str = " WHERE true;";
const COLUMN_DRAFT: &str = "SELECT n.c FROM warehouse.travel.flights n;";
const COLUMN_SUFFIX: &str = " FROM warehouse.travel.flights n;";

pub(super) async fn exercise(address: &str) {
    let address = address.replace("grpc+tcp://", "http://");
    tokio::task::spawn_blocking(move || {
        let directory =
            std::env::temp_dir().join(format!("sql-bomb-completion-{}", std::process::id()));
        fs::create_dir(&directory).expect("the test home is isolated");
        let mut terminal = Terminal::start(&directory, &["--flightsql"]);
        terminal.wait("connection form", |screen| {
            screen.contains("Connection address")
        });
        terminal.send("\x01");
        terminal.paste(&address);
        terminal.send("\t");
        terminal.paste(crate::HEADERS);
        terminal.send("\t\t\r");
        terminal.wait("authenticated schema", |screen| {
            screen.contains("3 tables available")
        });
        accepts_tables_and_columns(&mut terminal);
        dismisses_without_executing(&mut terminal);
        terminal.close();
        rejects_metadata_without_blocking_editor(&directory, &address);
        fs::remove_dir_all(directory).expect("the test can remove its isolated home");
    })
    .await
    .expect("the terminal interaction succeeds");
}

fn accepts_tables_and_columns(terminal: &mut Terminal) {
    for (draft, suffix, suggestion) in [
        (TABLE_DRAFT, TABLE_SUFFIX, "flights"),
        (COLUMN_DRAFT, COLUMN_SUFFIX, "city"),
    ] {
        terminal.send("\x01");
        terminal.paste(draft);
        terminal.wait("SQL draft", |screen| screen.contains(draft));
        terminal.send(&"\x1b[D".repeat(suffix.chars().count()));
        terminal.wait("schema suggestion", |screen| screen.contains(suggestion));
        if suggestion == "flights" {
            terminal.send("\t");
        } else {
            terminal.click(suggestion);
        }
        terminal.send("!");
        let expected = match suggestion {
            "flights" => "SELECT * FROM warehouse.travel.flights! WHERE true;",
            _ => "SELECT n.city! FROM warehouse.travel.flights n;",
        };
        let screen = terminal.wait("completion and caret", |screen| screen.contains(expected));
        assert_eq!(query_line(&screen), expected);
    }
}

fn dismisses_without_executing(terminal: &mut Terminal) {
    const DRAFT: &str = "SELECT * FROM warehouse.travel.fl";
    terminal.send("\x01");
    terminal.paste(DRAFT);
    terminal.wait("SQL draft", |screen| screen.contains(DRAFT));
    terminal.wait("table suggestions", |screen| screen.contains("flights"));
    terminal.send("\x1b");
    let screen = terminal.wait("dismissed suggestions", |screen| {
        !screen.contains("flights")
    });
    assert_eq!(query_line(&screen), DRAFT);
    terminal.send("\r");
    terminal.paste("-- second line");
    let screen = terminal.wait("editor newline", |screen| screen.contains("-- second line"));
    assert_eq!(query_line(&screen), DRAFT);
    let second_line = screen
        .lines()
        .skip_while(|line| !line.contains(DRAFT))
        .nth(1)
        .expect("the editor displays a second line");
    assert_eq!(
        second_line.trim().trim_matches('│').trim(),
        "-- second line"
    );
    terminal.send("\x10");
    let screen = terminal.wait("empty query history", |screen| screen.contains("0 queries"));
    let count = screen
        .lines()
        .find(|line| line.contains("0 queries"))
        .expect("the history count is visible")
        .split('│')
        .map(str::trim)
        .find(|text| text.ends_with("queries"))
        .expect("the history pane displays a query count");
    assert_eq!(count, "0 queries");
}

fn rejects_metadata_without_blocking_editor(directory: &std::path::Path, address: &str) {
    let mut terminal = Terminal::start(directory, &["--flightsql", address]);
    terminal.wait("schema authentication error", |screen| {
        screen.contains("Suggestions unavailable:") && screen.contains("Missing request headers")
    });
    terminal.paste("SELECT 1;");
    let screen = terminal.wait("editable query", |screen| screen.contains("SELECT 1;"));
    assert_eq!(query_line(&screen), "SELECT 1;");
    terminal.close();
}

fn query_line(screen: &str) -> &str {
    screen
        .lines()
        .find(|line| line.contains("SELECT "))
        .expect("the SQL editor is visible")
        .trim()
        .trim_matches('│')
        .trim()
}
