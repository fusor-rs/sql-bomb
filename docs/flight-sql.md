# Clients and Flight SQL

A client is the part of sql-bomb that connects to a server and runs queries.
Flight SQL is currently the only client; PostgreSQL and MySQL clients are not
implemented. Arrow Flight SQL sends SQL over gRPC and receives results in Apache
Arrow format, which preserves each column's data type.

## Connection address

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

## Authentication

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

## Remembering headers

Headers stay in memory unless you check **Remember headers securely** when saving
a connection. This option is off by default. It stores headers in macOS Keychain
or Linux Secret Service. On Linux, the Secret Service keyring must be unlocked
and available on your session D-Bus.

Without that option, reopening a saved connection that used headers asks for
them again. Renaming the connection without entering headers keeps that prompt.
Credential-store failures appear in the form. Headers are never saved in query
history or written to a plaintext fallback file.
