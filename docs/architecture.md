# How it works

This page describes the implementation for contributors. The
[workspace guide](workspace.md) covers using the app.

## Clients and results

Each client implements the public [QueryClient trait](../src/lib.rs), the shared
Rust interface for running queries. It returns the columns and their types,
followed by a stream of Arrow record batches: chunks of rows stored by column.
Errors can arrive with the stream. The terminal displays a window of those rows
and columns while keeping received rows in memory.

The [client registry](../src/client.rs) supplies the chooser, CLI flags, and
connection setup. Network operations run in Tokio, Rust's asynchronous runtime,
so the interface can respond while results arrive.

Each Flight SQL query opens a gRPC connection, executes a statement, and reads all
returned endpoints in order. When an endpoint advertises multiple locations,
sql-bomb uses the first. The optional `QueryClient::tables()` method uses the
Flight SQL `CommandGetTables` operation with schemas included. It returns each
table's catalog, database schema, name, type, and Arrow column schema.

## Terminal interface

Fusor connects the [HTML template](../ui/app.html) to
[Rust state](../src/terminal.rs), updating the view when that state changes.
Hypercmd handles terminal layout, focus, and input.

## SQL suggestions

The independent [completion crate](../crates/completion) uses Tree-sitter, a
parser library, to read SQL around the cursor. It combines that syntax with table
and column information from the client and returns text edits to the editor.
It runs inside sql-bomb; it does not start a language server or execute SQL.
