use arrow_array::{
    ArrayRef, BinaryArray, BooleanArray, Date32Array, Float32Array, Float64Array, Int16Array,
    Int32Array, Int64Array, RecordBatch, StringArray, Time64MicrosecondArray,
    TimestampMicrosecondArray, UInt32Array, cast::AsArray, types::Int32Type,
};
use arrow_schema::{DataType, Field, Schema};
use futures_util::TryStreamExt;
use sql_bomb::{Error, Postgres, QueryClient, Table};
use std::{
    fs, net::TcpListener, os::unix::fs::PermissionsExt, path::PathBuf, process::Command, sync::Arc,
};
use tempfile::TempDir;
use tokio_postgres::error::SqlState;

const USER: &str = "sql_bomb";
const PASSWORD: &str = "fixture password";
const SETUP: &[&str] = &[
    "CREATE SCHEMA travel",
    "CREATE TABLE travel.samples (
    id int4 NOT NULL,
    boolean bool,
    int16 int2,
    int64 int8,
    oid oid,
    float32 float4,
    float64 float8,
    text varchar(10),
    bytes bytea,
    uuid uuid,
    json json,
    jsonb jsonb,
    date date,
    time time,
    timestamp timestamp,
    timestamptz timestamptz,
    tags text[]
)",
    r#"INSERT INTO travel.samples VALUES (
    1, true, -2, 9000000000, 4, 1.5, -2.25, 'café', '\x00ff',
    'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11', '{"b": 1, "a": [true]}', '{"b": 1, "a": [true]}',
    '1970-01-02', '12:34:56.789', '2000-01-01 00:00:01', '2000-01-01 00:00:00+01', '{x}'
)"#,
    "CREATE VIEW travel.arrivals AS SELECT text AS city FROM travel.samples",
];
const SAMPLES: &str = "SELECT id, boolean, int16, int64, oid, float32, float64, text, bytes,
    uuid, json, jsonb, date, time, timestamp, timestamptz
FROM travel.samples";
const NUMBERS: &str = "SELECT value
FROM (VALUES (-1234.5600::numeric), (0.00012), (10000), ('NaN'), (NULL)) AS numbers(value)";
const UUID: &str = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11";
const JSON: &str = r#"{"b": 1, "a": [true]}"#;
const JSONB: &str = r#"{"a": [true], "b": 1}"#;
const SERIES: &str = "SELECT generate_series(1, 3000) AS number";
const SERIES_LENGTH: i32 = 3000;
const MISSING_TABLE_QUERY: &str = "SELECT * FROM missing_table";
const PRIVATE_KEY_MODE: u32 = 0o600;
const RSA_KEY: &str = "rsa:2048";
const SERVER_EXTENSIONS: &str = "basicConstraints = CA:FALSE
subjectAltName = DNS:localhost
";
const ENCRYPTED_QUERY: &str = "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()";
const UNTRUSTED_CERTIFICATE: &str = "PostgreSQL request failed; check the query, credentials, and \
    server: error performing TLS handshake: invalid peer certificate: UnknownIssuer";
const ARRAY_QUERY: &str = "SELECT tags FROM travel.samples";

struct Server {
    directory: TempDir,
    address: String,
}

impl Server {
    fn start() -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory is available");
        let password = directory.path().join("password");
        fs::write(&password, PASSWORD).expect("the temporary directory is writable");
        run(Command::new("initdb")
            .arg("--pgdata")
            .arg(data(&directory))
            .args(["--username", USER, "--auth", "scram-sha-256", "--no-locale"])
            .arg("--pwfile")
            .arg(&password));
        issue_server_certificate(&directory);
        let port = TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("a loopback port is available")
            .port();
        let options = format!(
            "-p {port} -c listen_addresses=127.0.0.1 -c unix_socket_directories='' -c ssl=on"
        );
        run(pg_ctl(&directory)
            .args(["--wait", "--options", &options, "--log"])
            .arg(directory.path().join("server.log"))
            .arg("start"));
        Self {
            directory,
            address: format!("postgres://{USER}@127.0.0.1:{port}/postgres"),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        run(pg_ctl(&self.directory).args(["--mode", "immediate", "stop"]));
    }
}

fn issue_server_certificate(directory: &TempDir) {
    let path = |name: &str| directory.path().join(name);
    let key = data(directory).join("server.key");
    fs::write(path("server.ext"), SERVER_EXTENSIONS).expect("the temporary directory is writable");
    run(Command::new("openssl")
        .args(["req", "-x509", "-newkey", RSA_KEY, "-nodes", "-days", "1"])
        .args(["-subj", "/CN=sql-bomb test CA", "-keyout"])
        .arg(path("ca.key"))
        .arg("-out")
        .arg(path("ca.crt")));
    run(Command::new("openssl")
        .args([
            "req",
            "-new",
            "-newkey",
            RSA_KEY,
            "-nodes",
            "-subj",
            "/CN=localhost",
        ])
        .arg("-keyout")
        .arg(&key)
        .arg("-out")
        .arg(path("server.csr")));
    run(Command::new("openssl")
        .args(["x509", "-req", "-days", "1", "-set_serial", "1", "-in"])
        .arg(path("server.csr"))
        .arg("-CA")
        .arg(path("ca.crt"))
        .arg("-CAkey")
        .arg(path("ca.key"))
        .arg("-extfile")
        .arg(path("server.ext"))
        .arg("-out")
        .arg(data(directory).join("server.crt")));
    fs::set_permissions(&key, fs::Permissions::from_mode(PRIVATE_KEY_MODE))
        .expect("the server key is writable");
}

fn data(directory: &TempDir) -> PathBuf {
    directory.path().join("data")
}

fn pg_ctl(directory: &TempDir) -> Command {
    let mut command = Command::new("pg_ctl");
    // PostgreSQL on macOS refuses to start without a valid locale in the environment.
    command
        .env("LC_ALL", "C")
        .arg("--pgdata")
        .arg(data(directory));
    command
}

fn run(command: &mut Command) {
    let output = command
        .output()
        .expect("PostgreSQL server binaries (initdb, pg_ctl) and openssl must be on PATH");
    assert!(
        output.status.success(),
        "{command:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn rows(client: &dyn QueryClient, sql: &str) -> (Schema, Vec<RecordBatch>) {
    let query = client
        .query(sql.into())
        .await
        .expect("the fixture SQL runs");
    let batches = query.batches.try_collect().await;
    (query.schema, batches.expect("the fixture rows decode"))
}

#[tokio::test]
async fn postgres_streams_typed_rows_discovers_tables_and_reports_errors() {
    let server = Server::start();
    let client = Postgres::new(&server.address, PASSWORD).unwrap();
    let client: &dyn QueryClient = &client;
    for statement in SETUP {
        assert_eq!(rows(client, statement).await.1, vec![]);
    }
    let samples = assert_typed_rows(client).await;
    assert_numeric_text(client).await;
    assert_rows_span_batches(client).await;
    assert_table_discovery(client, &samples).await;
    assert_errors(client, &server.address).await;
    assert_ssl_modes(&server.address).await;
}

async fn assert_typed_rows(client: &dyn QueryClient) -> Schema {
    let columns: Vec<(&str, ArrayRef)> = vec![
        ("id", Arc::new(Int32Array::from(vec![1]))),
        ("boolean", Arc::new(BooleanArray::from(vec![true]))),
        ("int16", Arc::new(Int16Array::from(vec![-2]))),
        ("int64", Arc::new(Int64Array::from(vec![9_000_000_000]))),
        ("oid", Arc::new(UInt32Array::from(vec![4]))),
        ("float32", Arc::new(Float32Array::from(vec![1.5]))),
        ("float64", Arc::new(Float64Array::from(vec![-2.25]))),
        ("text", Arc::new(StringArray::from(vec!["café"]))),
        (
            "bytes",
            Arc::new(BinaryArray::from(vec![[0, 255].as_slice()])),
        ),
        ("uuid", Arc::new(StringArray::from(vec![UUID]))),
        ("json", Arc::new(StringArray::from(vec![JSON]))),
        ("jsonb", Arc::new(StringArray::from(vec![JSONB]))),
        ("date", Arc::new(Date32Array::from(vec![1]))),
        (
            "time",
            Arc::new(Time64MicrosecondArray::from(vec![45_296_789_000])),
        ),
        (
            "timestamp",
            Arc::new(TimestampMicrosecondArray::from(vec![946_684_801_000_000])),
        ),
        (
            "timestamptz",
            Arc::new(
                TimestampMicrosecondArray::from(vec![946_681_200_000_000]).with_timezone("+00:00"),
            ),
        ),
    ];
    let expected = columns
        .into_iter()
        .map(|(name, values)| (name, values, true));
    let expected = RecordBatch::try_from_iter_with_nullable(expected)
        .expect("fixture columns have one row each");
    let (schema, batches) = rows(client, SAMPLES).await;
    assert_eq!(schema, *expected.schema());
    assert_eq!(batches, vec![expected]);
    schema
}

async fn assert_numeric_text(client: &dyn QueryClient) {
    let numbers = StringArray::from(vec![
        Some("-1234.5600"),
        Some("0.00012"),
        Some("10000"),
        Some("NaN"),
        None,
    ]);
    let numbers = RecordBatch::try_from_iter([("value", Arc::new(numbers) as ArrayRef)])
        .expect("one column forms a batch");
    assert_eq!(rows(client, NUMBERS).await.1, vec![numbers]);
}

async fn assert_rows_span_batches(client: &dyn QueryClient) {
    let streamed: Vec<i32> = rows(client, SERIES)
        .await
        .1
        .iter()
        .flat_map(|batch| {
            batch
                .column(0)
                .as_primitive::<Int32Type>()
                .values()
                .to_vec()
        })
        .collect();
    assert_eq!(streamed, (1..=SERIES_LENGTH).collect::<Vec<_>>());
}

async fn assert_table_discovery(client: &dyn QueryClient, samples: &Schema) {
    let tables = client
        .tables()
        .expect("PostgreSQL supports table discovery")
        .await
        .expect("table discovery returns the fixture tables");
    let mut sample_fields: Vec<_> = samples.fields().to_vec();
    sample_fields[0] = Arc::new(Field::new("id", DataType::Int32, false));
    let expected = [
        (
            "arrivals",
            "VIEW",
            vec![Arc::new(Field::new("city", DataType::Utf8, true))],
        ),
        ("samples", "TABLE", sample_fields),
    ]
    .into_iter()
    .map(|(name, table_type, fields)| Table {
        catalog: None,
        database_schema: Some("travel".into()),
        name: name.into(),
        table_type: table_type.into(),
        schema: Schema::new(fields),
    })
    .collect::<Vec<_>>();
    assert_eq!(tables, expected);
}

async fn assert_errors(client: &dyn QueryClient, address: &str) {
    let unsupported = client.query(ARRAY_QUERY.into()).await.err();
    let unsupported = unsupported.expect("text[] columns are unsupported");
    assert_eq!(
        unsupported.to_string(),
        "Column tags has unsupported PostgreSQL type _text; cast it, as in tags::text"
    );
    let missing = client.query(MISSING_TABLE_QUERY.into()).await.err();
    assert_eq!(
        missing.expect("the table does not exist").to_string(),
        "PostgreSQL request failed; check the query, credentials, and server: \
         db error: ERROR: relation \"missing_table\" does not exist"
    );
    let wrong_password = Postgres::new(address, "wrong").expect("the fixture address is valid");
    let Some(Error::Postgres(failure)) = wrong_password.query(SERIES.into()).await.err() else {
        panic!("Expected the server's authentication error");
    };
    assert_eq!(failure.code(), Some(&SqlState::INVALID_PASSWORD));
    let url_password = address.replace(USER, &format!("{USER}:{PASSWORD}"));
    assert!(matches!(
        Postgres::new(&url_password, ""),
        Err(Error::PostgresPassword)
    ));
}

async fn assert_ssl_modes(address: &str) {
    for (parameters, encrypted) in [
        ("", true),
        ("?sslmode=require", true),
        ("?sslmode=disable", false),
    ] {
        let client = Postgres::new(&format!("{address}{parameters}"), PASSWORD)
            .expect("the fixture address is valid");
        let (_, batches) = rows(&client, ENCRYPTED_QUERY).await;
        assert_eq!(batches[0].column(0).as_boolean().value(0), encrypted);
    }
    for parameters in ["?sslmode=verify-ca", "?sslmode=verify-full"] {
        let client = Postgres::new(&format!("{address}{parameters}"), PASSWORD)
            .expect("the fixture address is valid");
        let failure = client.query(ENCRYPTED_QUERY.into()).await.err();
        assert_eq!(
            failure
                .expect("the self-signed certificate is untrusted")
                .to_string(),
            UNTRUSTED_CERTIFICATE
        );
    }
}
