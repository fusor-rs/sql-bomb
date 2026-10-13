# Clients

A client is the part of sql-bomb that connects to a server and runs queries. The
editor, results, saved connections, and query library work the same way with every
client. Available clients are listed below.

## Flight SQL

Arrow Flight SQL sends SQL over gRPC and receives results in Apache Arrow format,
which preserves each column's data type.

### Connection address

Choose **Flight SQL** in the client chooser, or pass its flag:

```sh
boom --flightsql
boom --flightsql http://localhost:50051
```

The first command opens the connection form. The second also skips the form, so
use it for connections without headers. An address without the flag, such as
`boom http://localhost:50051`, fills in the form after you choose a client.

The server URL contains a scheme, host, and port, such as
`http://localhost:50051` or `https://sql.example.com:443`. Credentials, paths, and
query parameters do not belong in the URL. Use `http://` or `grpc+tcp://` for an
unencrypted connection. Use `https://` or `grpc+tls://` for TLS encryption with
certificates trusted by your operating system.

### Authentication

Headers are extra name/value pairs sent with a request, often to carry a token.
Leave the field empty if your server needs none. Otherwise, enter one header per
line as `name: value`:

```text
authorization: Bearer YOUR_TOKEN
x-tenant: YOUR_TENANT
```

Header names and values must be ASCII; binary metadata is not supported.
Authentication uses these headers. Flight handshake authentication and client
certificates (mutual TLS) are not implemented.

A Flight SQL server can direct sql-bomb to another address to fetch results.
Headers are sent only to addresses with the same scheme, host, and port as the
configured server. See the
[Arrow Flight protocol](https://arrow.apache.org/docs/format/Flight.html) for the
protocol's endpoint and authentication conventions.

### Remembering headers

Headers stay in memory unless you check **Remember headers securely** when saving
a connection. This option is off by default. It stores headers in macOS Keychain
or Linux Secret Service. On Linux, the Secret Service keyring must be unlocked
and available on your session D-Bus.

Without that option, reopening a saved connection that used headers asks for
them again. Renaming the connection without entering headers keeps that prompt.
Credential-store failures appear in the form. Headers are never saved in query
history or written to a plaintext fallback file.

## PostgreSQL

Choose **PostgreSQL** in the client chooser, or use its flag. Adding a connection
address also skips the form, so use it for servers that need no password:

```sh
boom --postgres
boom --postgres postgres://postgres@localhost:5432/postgres
```

### Connection address

The address is a PostgreSQL URL with the user, host, port, and database, such as
`postgres://postgres@localhost:5432/postgres`. Connection parameters, such as
`sslmode` or `connect_timeout`, go in its query string. Key-value strings, such as
`host=localhost user=postgres dbname=postgres`, are also accepted.

Addresses are saved in plaintext with your query history, so they cannot contain a
password. Enter it in the **Password** field, or leave the field empty for servers
that do not ask for one. **Remember password securely** stores it in macOS
Keychain or Linux Secret Service, as described for Flight SQL headers; otherwise,
reopening a saved connection asks for the password again.

`sslmode` follows psql's rules. The default, `prefer`, uses TLS when the server
offers it and connects unencrypted otherwise; `require` refuses unencrypted
connections; `disable` never uses TLS. These modes encrypt without checking the
server's certificate, so servers with their own certificate authority, such as
Supabase, connect without setup, but an attacker could impersonate the server.
`verify-ca` also checks that a certificate authority trusted by your operating
system issued the certificate, and `verify-full` also checks that the certificate
names the host. The `allow` mode and `sslrootcert` are not supported.

### Queries and types

Each query opens a connection, prepares one statement, and streams its rows as
they arrive. Run one statement at a time; statements with parameters such as `$1`
are not supported. Each query runs on its own connection, so session settings and
transactions do not carry over to the next query.

| PostgreSQL type | Arrow type |
| --- | --- |
| `boolean` | Boolean |
| `smallint`, `integer`, `bigint` | Int16, Int32, Int64 |
| `oid` | UInt32 |
| `real`, `double precision` | Float32, Float64 |
| `text`, `varchar`, `char`, `name`, `citext` | Utf8 |
| `numeric` | Utf8, with every digit the server returns |
| `uuid`, `json`, `jsonb` | Utf8 |
| `bytea` | Binary |
| `date` | Date32 |
| `time` | Time64 in microseconds |
| `timestamp`, `timestamptz` | Timestamp in microseconds; `timestamptz` in UTC |

A query that returns another type, such as an array, `interval`, or enum, fails
with an error naming the column. Cast the column to text, as in `tags::text`.
Infinite dates and timestamps also need a text cast.

SQL suggestions include tables, views, materialized views, and foreign tables
outside `pg_catalog`, `information_schema`, and other `pg_` schemas, qualified by
schema. Columns of the built-in types above are suggested; `citext` comes from an
extension, so its columns are not.
