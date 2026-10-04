# sql-bomb

sql-bomb is a terminal application for running SQL and streaming results into a
table. You can inspect complete cell values and return to queries from your local
history or starred library. The installed executable is named `boom`.

Flight SQL is currently the only supported connection type. It connects to
servers that implement Arrow Flight SQL, a protocol for running SQL over gRPC.
Direct PostgreSQL and MySQL connections are not implemented.

The terminal interface is built with
[Hypercmd](https://github.com/fusor-rs/hypercmd) and
[Fusor](https://github.com/fusor-rs/fusor).

**In development · v0.1** — See [available clients](flight-sql.md) and
[status and known limitations](status.md).

[Get started](get-started.md) · [Using sql-bomb](workspace.md) ·
[Clients](flight-sql.md) · [How it works](architecture.md) · [Status](status.md) ·
[Contributing](../README.md#contributing)
