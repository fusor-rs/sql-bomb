# Status and known limitations

sql-bomb is in early development. Queries currently target statements that return
rows, such as `SELECT`. Client availability and protocol-specific limits are
listed under [Clients](flight-sql.md).

| Area | Included |
| --- | --- |
| Query workspace | Multiline SQL editor, searchable action menu, keyboard and mouse controls |
| SQL suggestions | Qualified tables and columns from referenced tables and aliases |
| Results | Streaming table, cancellation, and full cell inspection |
| Query library | Local history and starred queries in SQLite |
| Saved connections | Searchable saved connections; optional OS credential storage |
| Terminal platforms | macOS and Linux |

All received rows stay in memory until another query starts or the connection
changes. **Cancel** stops receiving results. It does not send a server-side SQL
cancellation command, so the server decides whether to keep running the query.
