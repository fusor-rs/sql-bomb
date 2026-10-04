# Using sql-bomb

## Saved connections

Check **Save on this device** in the connection form. Give it a name
(the hostname is suggested), then choose **Save & open**. Leave the checkbox
unchecked for a session-only connection. A direct CLI launch remains unsaved;
**Menu → Save connection…** saves it without clearing SQL or results.

When saved connections exist, launching `boom` opens **Connections**. Search
by name, client, or address; recently opened connections appear first. Click a row,
or use Up/Down and Enter, to open it. **New connection**, **Edit**, and **Remove**
are visible actions. Editing requires **Save changes**; Escape discards the edit.
Removal asks for confirmation and keeps query history, stars, and the active workspace.

Saved connections live in `~/.sqlbomb/queries.sqlite3`. Authentication and credential
storage options are described in the [client settings](flight-sql.md).

## The menu

**Menu** is always visible in the header. Click it or press Ctrl+K, then search
for an action. Click a result to open it, or use Up/Down and Enter. Unavailable
actions explain what is needed when selected.

Press `/` in results, previews, or other non-editing controls to open the same
menu. In text fields, `/` remains ordinary text. Search for an action by name, such as
`history`; a leading `/` is optional. Escape restores the previous pane, including the editor's
cursor and selection. Queries continue streaming while the menu is open.

## SQL suggestions

Clients that implement table discovery provide suggestions as you type. After
`FROM` or `JOIN`, the editor suggests tables from the active connection, including
catalog and schema qualifiers. In column positions, it suggests columns from
referenced tables and aliases, with their data types and source tables.

Use Up/Down to choose a suggestion, then Tab or a click to insert it. Insertion
replaces the identifier at the caret and preserves the rest of the query. Escape
closes suggestions; Enter inserts a newline. Suggestions stay out of comments
and string literals. Quoted identifiers preserve their quoting style.
The list shows a bounded set of matches; keep typing to narrow it.

Table and column information is loaded when you open the workspace and kept in
memory. **Refresh schema** reloads it after database changes or a discovery
failure. Loading and failures appear beneath the editor; typing and executing SQL
remain available. Clients without discovery work without suggestions.

Completion resolves catalog tables and aliases within the current statement or
subquery. Columns derived from `WITH` queries (CTEs), subquery select lists, and
set operations such as `UNION` are not resolved. Ambiguous table references do not
produce guessed column lists.

## Results

The multiline editor preserves pasted SQL, including comments and string
literals. Results appear as rows arrive. Column headers stay visible,
the selected cell is highlighted, and the footer shows your row and column.
Use the arrow keys to browse or the mouse wheel to scroll.

Long values end with an ellipsis. Enter opens the complete selected value and its
data type in a scrollable inspector. Large values are split into bounded parts
with Previous/Next controls. **Error details** opens the complete query error.
HTTP(S) cells show **Open link ↗**; a normal left click opens the full destination
in your browser.

**Cancel** stops receiving rows and keeps the results already received. While a
query streams, your selection stays put; End jumps to the latest row.

## History and starred queries

**Library** opens searchable **History** and **Starred** tabs. Filter by SQL or
connection address, browse the list, and preview the selected query. Each entry
includes its original server address and local timestamp. The panes stack in narrow
terminals.

**Use query** or Enter loads the full SQL into your current connection's editor
without executing it. Escape returns to your unchanged draft. **Star** or Ctrl+S
saves the editor's query, including SQL you have not run. In the library, it
stars or unstars the selected query. A star belongs to the exact SQL and server address;
editing that SQL creates a different query to star.

History and stars live in `~/.sqlbomb/queries.sqlite3`, outside the project.
Every run attempt is recorded before contacting the server, including failed and
cancelled queries. Repeated executions remain separate history entries. The
library survives restarts and supports multiple sql-bomb sessions.

New directories and database files are private to your account. The database
stores full SQL and server addresses in plaintext, with no automatic history expiry.
Storage errors are displayed rather than silently dropping the record.

## Keyboard and mouse

The Menu lists actions and their shortcuts. The full reference is below.

| Key | Action |
| --- | --- |
| Ctrl+K or Menu | Open or close the searchable action menu |
| / outside text fields | Open the menu |
| Command+Enter in SQL or Ctrl+R | Run the SQL in the editor |
| Ctrl+E / Ctrl+L | Focus SQL / results |
| Ctrl+P | Open the query library / focus its search |
| Ctrl+S | Star or unstar the editor's query / selected library query |
| Enter in library search or list | Load the selected query into the editor |
| Up / Down, then Enter in Connections | Choose and open a saved connection |
| Space on a checkbox | Toggle saving or remembering headers |
| Enter in SQL | Insert a newline |
| Ctrl+A in SQL | Select the entire query |
| Tab or Shift+Tab | Move between controls and panes |
| Arrows in results | Select a cell and reveal it when outside the window |
| PageUp / PageDown | Move one page through results or the inspector |
| Up / Down in library | Select the previous / next query |
| PageUp / PageDown in library | Browse one page of queries |
| Home / End in results | Select the first / last received row |
| Enter in results | Inspect the selected cell |
| Mouse wheel | Scroll the pane under the pointer |
| Shift+wheel or horizontal wheel | Move through result columns |
| Escape | Go back from connection setup, library, or inspector; otherwise cancel a running query |
| Ctrl+C | Quit and restore the terminal |

Command+Enter works only if your terminal forwards the Command key through
[enhanced keyboard reporting](https://sw.kovidgoyal.net/kitty/keyboard-protocol/).
If your terminal reserves that shortcut, map it to `ESC [ 13 ; 9 u` (without
spaces), or use Ctrl+R. Link clicks use `open` on macOS or `xdg-open` on Linux.
Modifier-click gestures belong to the terminal emulator.
