<p>
  <picture>
    <source media="(prefers-color-scheme: dark)"
            srcset="assets/brand/sql-bomb-horizontal-dark.svg">
    <img src="assets/brand/sql-bomb-horizontal.svg" alt="sql-bomb" width="300">
  </picture>
</p>

# Query SQL. Watch the rows arrive.

sql-bomb is a database-agnostic terminal application for running SQL and streaming
results into a table. You can inspect complete cell values, reopen saved
connections, and return to queries from your local history or starred library.
The installed executable is named `boom`.

A client connects sql-bomb to a database server, and every client shares the same
editor, results table, saved connections, and query library. The
[client reference](#clients) lists the available clients and their settings.

The terminal interface is built with
[Hypercmd](https://github.com/fusor-rs/hypercmd) and
[Fusor](https://github.com/fusor-rs/fusor).

**In development · v0.1** — See [available clients](#clients) and
[status and known limitations](#status-and-known-limitations).

[Get started](#get-started) · [Using sql-bomb](#using-sql-bomb) ·
[Clients](#clients) · [How it works](#how-it-works) · [Status](#status-and-known-limitations) ·
[Contributing](#contributing)

## Get started

sql-bomb runs in a terminal on macOS or Linux. To run queries, you need a running
database server and its address; the [client reference](#clients) lists the
connection types sql-bomb supports. sql-bomb connects to that server; it does not
start a database for you.

### Install with Cargo

Cargo is Rust's package manager, included when you install Rust through
[rustup](https://rustup.rs). After a sql-bomb release is published to crates.io,
you can install the `boom` executable with:

```sh
cargo install sql-bomb --locked
boom
```

`--locked` uses the dependency versions recorded in the release. Cargo installs
`boom` in `~/.cargo/bin` by default; that directory needs to be in your shell's PATH.

### Install a binary release

The shell installer downloads the latest published GitHub release for x86-64 or
ARM64 on Linux or macOS. It verifies the download's SHA-256 checksum and installs
`boom` in `~/.sqlbomb/bin`, without sudo or a Rust toolchain:

```sh
curl -fsSL https://boom.fusor.build/install.sh | sh
export PATH="$HOME/.sqlbomb/bin:$PATH"
boom
```

Add the `export` line to your shell profile, such as `~/.zshrc` or `~/.bashrc`, so
new terminals can find `boom`. Run `boom upgrade` to upgrade. To select a
release, append `-s -- v0.1.0` to `sh`. Set `SQL_BOMB_INSTALL` on the `sh` command
to choose a different install directory; the executable goes in its `bin/`.
Saved connections, query history, and stars are left untouched.

This method requires a GitHub release with completed binary uploads.

### Upgrade

Run this to update to the latest stable version:

```sh
boom upgrade
```

If you're already up to date, nothing happens. Your saved connections, query
history, and stars stay in place.

To check which version you're running, use `boom --version`.

### Connect to a server

1. Run `boom` and choose a client from the chooser.
2. Enter your server URL, including its scheme and port, such as
   `http://localhost:50051`. This example requires a server already running at that
   address. Use `https://` for a server with TLS encryption.
3. If the server requires authentication, enter its request headers, such as
   `authorization: Bearer YOUR_TOKEN`. Otherwise, leave the headers empty.
4. Select **Open workspace**.

Type a query in the editor and press Ctrl+R, or click **Run**:

```sql
SELECT 1 AS value;
```

Rows appear below the editor as the server sends them. Press Ctrl+C to quit.

Once you save a connection, `boom` opens your saved connections instead. See
[Using sql-bomb](#using-sql-bomb) for saving connections and browsing results, or the
[client reference](#clients) for supported URLs, authentication, and
command-line shortcuts that skip the chooser and form.

### Build from source

With Rust and Cargo installed, clone the repository and install from the checkout:

```sh
git clone https://github.com/fusor-rs/sql-bomb
cd sql-bomb
cargo install --path . --locked
boom
```

For development, `cargo run --locked` builds and launches the app from the checkout.

## Using sql-bomb

### Save and reopen connections

Check **Save on this device** in the connection form. Give it a name
(the hostname is suggested), then choose **Save & open**. Leave the checkbox
unchecked for a session-only connection. A direct CLI launch remains unsaved;
**Menu → Save connection…** saves it without clearing SQL or results.

When saved connections exist, launching `boom` opens **Connections**. Search
by name, client, or address; recently opened profiles appear first. Click a row,
or use Up/Down and Enter, to open it. **New connection**, **Edit**, and **Remove**
are visible actions. Editing requires **Save changes**; Escape discards the edit.
Removal asks for confirmation and keeps query history, stars, and the active workspace.

Profiles live in `~/.sqlbomb/queries.sqlite3`. Authentication and credential
storage options are described in the [client settings](#clients).

### Find an action

**Menu** is always visible in the header. Click it or press Ctrl+K, then search
for an action. Click a result to open it, or use Up/Down and Enter. Unavailable
actions explain what is needed when selected.

Press `/` in results, previews, or other non-editing controls to open the same
menu. In text fields, `/` remains ordinary text. The menu search accepts names
such as `/history`. Escape restores the previous pane, including the editor's
cursor and selection. Queries continue streaming while the menu is open.

### SQL suggestions

Clients that implement table discovery provide suggestions as you type. After
`FROM` or `JOIN`, the editor suggests tables from the active connection, including
catalog and schema qualifiers. In column positions, it suggests columns from
referenced tables and aliases, with their Arrow types and source tables.

Use Up/Down to choose a suggestion, then Tab or a click to insert it. Insertion
replaces the identifier at the caret and preserves the rest of the query. Escape
closes suggestions; Enter inserts a newline. Suggestions stay out of comments
and string literals. Quoted identifiers preserve their quoting style.
The list shows a bounded set of matches; keep typing to narrow it.

Schema metadata is held in memory for the active connection. **Refresh schema**
reloads it after database changes or a discovery failure. Loading and failures
appear beneath the editor; typing and executing SQL remain available. Clients
without discovery work without suggestions.

Completion resolves catalog tables and aliases within the current statement or
subquery. Columns derived from CTEs, subquery projections, and set operations are
not resolved. Ambiguous table references do not produce guessed column lists.

### Explore results as they arrive

The multiline editor preserves pasted SQL, including comments and string
literals. Results appear as rows arrive. Column headers stay visible,
the selected cell is highlighted, and the footer shows your row and column.
Use the arrow keys to browse or the mouse wheel to scroll.

Long values end with an ellipsis. Enter opens the complete selected value and its
Arrow type in a scrollable inspector. Large values are split into bounded parts
with Previous/Next controls. **Error details** opens the complete query error.
HTTP(S) cells show **Open link ↗**; a normal left click opens the full destination
in your browser. The label avoids terminal auto-detection of truncated URLs.

**Cancel** stops receiving rows and keeps the results already received. While a
query streams, your selection stays put; End jumps to the latest row.

### Keep your queries

**Library** opens searchable **History** and **Starred** tabs. Filter by SQL or
connection address, browse the list, and preview the selected query. Each entry
includes its original endpoint and local timestamp. The panes stack in narrow
terminals.

**Use query** or Enter loads the full SQL into your current connection's editor
without executing it. Escape returns to your unchanged draft. **Star** or Ctrl+S
saves the editor's query, including SQL you have not run. In the library, it
stars or unstars the selected query. A star belongs to the exact SQL and endpoint;
editing that SQL creates a different query to star.

History and stars live in `~/.sqlbomb/queries.sqlite3`, outside the project.
Every run attempt is recorded before contacting the server, including failed and
cancelled queries. Repeated executions remain separate history entries. The
library survives restarts and supports multiple sql-bomb sessions.

New directories and database files are private to your account. The database
stores full SQL and endpoints in plaintext, with no automatic history expiry.
Storage errors are displayed rather than silently dropping the record.

### Keyboard and mouse

The Menu lists actions and their shortcuts. The full reference is below.

<details>
<summary>Controls</summary>

| Key | Action |
| --- | --- |
| Ctrl+K / Menu | Open or close the searchable action menu |
| / outside text fields | Open the menu |
| Command+Enter in SQL / Ctrl+R | Run the SQL in the editor |
| Ctrl+E / Ctrl+L | Focus SQL / results |
| Ctrl+P | Open the query library / focus its search |
| Ctrl+S | Star or unstar the editor's query / selected library query |
| Enter in library search or list | Load the selected query into the editor |
| Up / Down, then Enter in Connections | Choose and open a saved connection |
| Space on a checkbox | Toggle saving or remembering headers |
| Enter in SQL | Insert a newline |
| Ctrl+A in SQL | Select the entire query |
| Tab / Shift+Tab | Move between controls and panes |
| Arrows in results | Select a cell and reveal it when outside the window |
| PageUp / PageDown | Move one page through results or the inspector |
| Up / Down in library | Select the previous / next query |
| PageUp / PageDown in library | Browse one page of queries |
| Home / End in results | Select the first / last received row |
| Enter in results | Inspect the selected cell |
| Mouse wheel | Scroll the pane under the pointer |
| Shift+wheel / horizontal wheel | Move through result columns |
| Escape | Go back from connection setup, library, or inspector; otherwise cancel a running query |
| Ctrl+C | Quit and restore the terminal |

Command+Enter requires the terminal to forward the Command key. Hypercmd requests
[enhanced keyboard reporting](https://sw.kovidgoyal.net/kitty/keyboard-protocol/).
If your terminal reserves that shortcut, map it to `ESC [ 13 ; 9 u` (without
spaces), or use Ctrl+R. Link clicks use `open` on macOS or `xdg-open` on Linux.
Modifier-click gestures belong to the terminal emulator.

</details>

## Clients

Clients provide connection settings and query execution. The editor, result
browser, saved connections, and query library share the same workspace regardless
of the selected client.

Available clients are listed below.

### Flight SQL

Choose **Flight SQL** in the client chooser, or use its flag to skip selection.
Adding a connection address also skips the form:

```sh
boom --flightsql
boom --flightsql http://localhost:50051
```

A bare address, such as `boom http://localhost:50051`, prefills the connection
form after you choose a client.

Flight SQL takes a server URL, including its scheme and port, such as
`http://localhost:50051` or `https://sql.example.com:443`. Use `http://` or
`grpc+tcp://` for plaintext and `https://` or `grpc+tls://` for TLS with system
roots. The form also accepts optional request headers, one `name: value` per line:

```text
authorization: Bearer YOUR_TOKEN
x-tenant: YOUR_TENANT
```

Server URLs contain only the scheme, host, and port; put authentication in headers,
not URL credentials, paths, or query parameters. Headers are sent to the configured
server and locations with the same scheme, host, and port. They are not forwarded
to other servers. Header names and values must be ASCII; binary metadata is not
supported. Authentication uses supplied headers; handshake authentication and
mutual TLS are not implemented. See the
[Arrow Flight protocol](https://arrow.apache.org/docs/format/Flight.html) for
endpoint and authentication conventions.

Headers stay in memory unless you check **Remember headers securely** when saving
a connection. This uses macOS Keychain or Linux Secret Service and is off by
default. Otherwise, reopening a profile that used headers asks for them again.
Renaming a session-only profile without entering headers keeps that prompt. Linux
credential storage requires an unlocked Secret Service provider on the session
D-Bus. Credential-store failures are shown in the form; headers are never saved
in query history or written to a plaintext fallback.

Each query opens a gRPC connection, executes a statement, and reads all returned
endpoints in order. For advertised endpoint locations, the first location is used.
The optional `QueryClient::tables()` API uses `CommandGetTables` with schemas
included. It returns each table's catalog, database schema, name, type, and Arrow
column schema.

## How it works

- **Pluggable clients.** The public [QueryClient trait](src/lib.rs) returns a
  schema and a fallible stream of Arrow record batches. The terminal consumes
  this interface; each client implements its own protocol. The
  [client registry](src/client.rs) supplies the chooser, CLI flags, and connection
  setup. Network work runs in a Tokio runtime.
- **Typed results.** Clients return Arrow record batches. The UI
  renders a window of the received rows and columns without discarding older rows.
- **HTML views, Rust state.** Fusor's reactive bindings connect the
  [HTML template](ui/app.html) to [Rust state](src/terminal.rs). Hypercmd handles
  terminal layout, focus, and input.
- **SQL completion.** The independent [completion crate](crates/completion)
  uses Tree-sitter's SQL grammar and client-provided metadata. It runs in process
  and returns text edits; it does not start a language server or execute SQL.

## Status and known limitations

sql-bomb is in early development. Queries currently target statements that return
rows. Client availability and protocol-specific limits are listed under [Clients](#clients).

| Capability | Status |
| --- | --- |
| Query workspace | Multiline SQL editor, searchable action menu, keyboard and mouse controls |
| SQL suggestions | Qualified tables and columns from referenced tables and aliases |
| Results | Streaming table, cancellation, and full cell inspection |
| Query library | Local history and starred queries in SQLite |
| Saved connections | Searchable local profiles; optional OS credential storage |
| Terminal platforms | macOS and Linux |

Received batches stay in memory until another query starts or the connection
changes. Cancellation closes the client stream; it does not send a server-side
SQL cancellation command.

## Contributing

Read [AGENTS.md](AGENTS.md) for the repository's coding and review rules. The
client registry lives in `src/client.rs`, client adapters in `src/clients/`,
terminal state in `src/terminal/`, and views in `ui/`.
Connection views live together in `ui/connections/`. `ui/app.html` composes them
with the workspace, library, inspector, and menu templates. Their shared terminal
input is registered in
`src/terminal/view.rs`; styles live in `ui/terminal.css`.
Dependency versions are pinned.

Run the required checks:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

The Flight SQL integration test checks delivery before completion, multiple
endpoints, request headers, nullable Unicode values, empty results, and errors.
It also launches `boom` in a terminal to verify metadata discovery, Tab and mouse
completion, caret placement, dismissal, and editing after a metadata failure.
UI tests cover client selection, saved connections, credential prompts, history,
stars, search, menu navigation, responsive layout, and draft recovery. Storage
tests check that profiles exclude secrets and failed saves preserve credentials.

The [Check workflow](.github/workflows/check.yml) runs these checks on Linux and
macOS for pushes and pull requests, then installs and launches the executable.

### Releases

Publish a GitHub release with a tag matching the package version in `Cargo.toml`,
such as `v0.1.0`. The [Release workflow](.github/workflows/release.yml) runs Check,
validates the tag, and builds Linux musl and macOS executables for x86-64 and ARM64.
Each archive contains `boom`, `LICENSE`, and `README.md` inside a directory
named `sql-bomb-VERSION-TARGET`. Archives use that name with `.tar.gz`, accompanied
by a `.tar.gz.sha256` checksum file. Each build uses [install.sh](install.sh) to
verify, install, and run the executable before any archives are attached to the
release. It also checks that a corrupt download leaves the installed executable
unchanged. `SQL_BOMB_DOWNLOAD_BASE` points the installer at these local archives
during CI.

The same release publishes `sql-bomb-completion` and `sql-bomb` to crates.io after
all binary builds pass. Configure a GitHub environment named `crates-io` with a
`CARGO_REGISTRY_TOKEN` secret authorized to publish both crates. The workflow
verifies the packaged workspace before uploading and skips versions already
published when a release is rerun.

After binary uploads and crate publication succeed, the workflow attaches
`install.sh` as the release's readiness marker. `boom upgrade` offers a newer
release only when its assets include `install.sh` and the user's platform
archive with its `.sha256` file; otherwise it reports that the release is still
being published.

Running Release manually produces workflow artifacts without publishing them.
Windows binaries and a PowerShell installer require Windows support in Hypercmd's
native runtime and the query library's
filesystem handling.

Logo assets and palette guidance are in [the brand guide](assets/brand/BRANDING.md).

### Website

The landing page and documentation are Fusor apps in a separate `apps/` workspace,
following Hypercmd's site setup. See [website development and Vercel deployment](apps/README.md)
for local previews, browser checks, and the manual **Deploy site** workflow.

## License

Licensed under the [MIT License](LICENSE).
