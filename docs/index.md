# sql-bomb

sql-bomb is a database-agnostic terminal application for running SQL and streaming
results into a table. You can inspect complete cell values, reopen saved
connections, and return to queries from your local history or starred library.
The installed executable is named `boom`.

A client connects sql-bomb to a database server, and every client shares the same
editor, results table, saved connections, and query library. The
[client reference](flight-sql.md) lists the available clients and their settings.

The terminal interface is built with
[Hypercmd](https://github.com/fusor-rs/hypercmd) and
[Fusor](https://github.com/fusor-rs/fusor).

**In development · v0.1** — See [available clients](flight-sql.md) and
[status and known limitations](status.md).

[Get started](get-started.md) · [Using sql-bomb](workspace.md) ·
[Clients](flight-sql.md) · [How it works](architecture.md) · [Status](status.md) ·
[Contributing](../README.md#contributing)
