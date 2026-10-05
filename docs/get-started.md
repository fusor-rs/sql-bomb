# Get started

sql-bomb runs in a terminal on macOS or Linux. To run queries, you need a running
database server and its address; the [client reference](flight-sql.md) lists the
connection types sql-bomb supports. sql-bomb connects to that server; it does not
start a database for you.

## Install with Cargo

Cargo is Rust's package manager, included when you install Rust through
[rustup](https://rustup.rs). After a sql-bomb release is published to crates.io,
you can install the `boom` executable with:

```sh
cargo install sql-bomb --locked
boom
```

`--locked` uses the dependency versions recorded in the release. Cargo installs
`boom` in `~/.cargo/bin` by default; that directory needs to be in your shell's PATH.

## Install a binary release

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

## Upgrade

Run this to update to the latest stable version:

```sh
boom upgrade
```

If you're already up to date, nothing happens. Your saved connections, query
history, and stars stay in place.

To check which version you're running, use `boom --version`.

## Connect to a server

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
[Using sql-bomb](workspace.md) for saving connections and browsing results, or the
[client reference](flight-sql.md) for supported URLs, authentication, and
command-line shortcuts that skip the chooser and form.

## Build from source

With Rust and Cargo installed, clone the repository and install from the checkout:

```sh
git clone https://github.com/fusor-rs/sql-bomb
cd sql-bomb
cargo install --path . --locked
boom
```

For development, `cargo run --locked` builds and launches the app from the checkout.
