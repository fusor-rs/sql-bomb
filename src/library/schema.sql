PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS saved_connections (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    client TEXT NOT NULL,
    address TEXT NOT NULL,
    credentials TEXT NOT NULL CHECK (credentials IN ('none', 'session', 'keyring')),
    opened_at INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS queries (
    id INTEGER PRIMARY KEY,
    endpoint TEXT NOT NULL,
    sql TEXT NOT NULL,
    starred_at INTEGER,
    UNIQUE (endpoint, sql)
);

CREATE TABLE IF NOT EXISTS history (
    id INTEGER PRIMARY KEY,
    query_id INTEGER NOT NULL REFERENCES queries (id),
    executed_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE VIEW IF NOT EXISTS library_entries AS
SELECT history.id, queries.endpoint, queries.sql,
       queries.starred_at IS NOT NULL AS starred,
       history.executed_at AS recorded_at, FALSE AS starred_only
FROM history
JOIN queries ON queries.id = history.query_id
UNION ALL
SELECT id, endpoint, sql, TRUE, starred_at, TRUE
FROM queries
WHERE starred_at IS NOT NULL;
