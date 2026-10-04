use sql_bomb_completion::{Column, Engine, Kind, Suggestion, Table};

const CASES: &[(&str, &[&str])] = &[
    ("SELECT * FROM ne|", &["atlas.benzinga.news"]),
    (
        "SELECT * FROM /* note */|",
        &["archive.public.orders", "atlas.benzinga.news"],
    ),
    (
        "SELECT ti| FROM news n JOIN unknown u ON n.title = u.title",
        &["n.title"],
    ),
    ("SELECT * FROM atlas.b|", &["benzinga.news"]),
    ("SELECT * FROM benzinga.ne|", &["news"]),
    ("SELECT * FROM atlas.benzinga.|", &["news"]),
    ("SELECT * FROM \"atlas\".\"benzinga\".\"ne|", &["\"news\""]),
    ("SELECT * FROM news JOIN ord|", &["archive.public.orders"]),
    ("SELECT ti| FROM atlas.benzinga.news n", &["title"]),
    ("SELECT n.ti| FROM atlas.benzinga.news n", &["title"]),
    ("SELECT N.ti| FROM atlas.benzinga.news n", &["title"]),
    ("SELECT * FROM news n WHERE n.ti|", &["title"]),
    ("SELECT * FROM news n WHERE n.or|", &["\"order\""]),
    ("SELECT * FROM news n WHERE n.\"Ré|", &["\"Résumé\""]),
    ("SELECT * FROM news n WHERE n.\"ti|\"", &["\"title\""]),
    (
        "SELECT * FROM news n WHERE n.\"quote\"\"|",
        &["\"quote\"\"name\""],
    ),
    ("SELECT * FROM news n WHERE n.`ti|`", &["`title`"]),
    ("SELECT * FROM news n WHERE n.[ti|]", &["[title]"]),
    (
        "SELECT ti| FROM news n; SELECT total FROM orders",
        &["title"],
    ),
    ("SELECT title FROM news; SELECT to| FROM orders", &["total"]),
    (
        "SELECT id| FROM news n JOIN orders o ON n.id = o.id",
        &["n.id", "o.id"],
    ),
    (
        "SELECT * FROM news n WHERE EXISTS (SELECT to| FROM orders)",
        &["total"],
    ),
    (
        "SELECT * FROM news n WHERE EXISTS (SELECT ti| FROM orders)",
        &[],
    ),
    ("SELECT ti| FROM missing", &[]),
    ("SELECT title FROM news; SELECT ti|", &[]),
    ("SELECT news.ti| FROM news n", &[]),
    ("SELECT x.ti| FROM news n", &[]),
    ("SELECT * FROM news WHERE 'ti|", &[]),
    ("SELECT * FROM news WHERE 'ti|tle'", &[]),
    ("SELECT * FROM news WHERE $$ti|", &[]),
    ("SELECT * FROM news WHERE $body$ti|$body$", &[]),
    ("SELECT * FROM news WHERE -- ti|", &[]),
    ("SELECT * FROM news WHERE /* ti|", &[]),
    ("SELECT * FROM news WHERE /* nested /* note */ ti| */", &[]),
    (
        "WITH news AS (SELECT id FROM orders) SELECT ti| FROM news",
        &[],
    ),
    ("SELECT ti| FROM news UNION SELECT total FROM orders", &[]),
];

#[test]
fn completes_catalog_names_in_the_current_query_scope() {
    let tables = catalog();
    let mut engine = Engine::new().unwrap();
    for (marked, expected) in CASES {
        let cursor = marked.find('|').unwrap();
        let sql = marked.replace('|', "");
        let suggestions = engine.complete(&sql, cursor, &tables).unwrap();
        let insertions = suggestions
            .iter()
            .map(|suggestion| suggestion.insertion.as_str())
            .collect::<Vec<_>>();
        assert_eq!(&insertions, expected, "{marked}");
    }
    assert_eq!(
        engine
            .complete("SELECT \"Ré\" FROM news", 11, &tables)
            .unwrap(),
        [Suggestion {
            label: "Résumé".to_owned(),
            detail: "Utf8 · atlas.benzinga.news".to_owned(),
            insertion: "\"Résumé\"".to_owned(),
            kind: Kind::Column,
            replacement: 7..12,
        }]
    );
    assert_eq!(
        engine
            .complete("SELECT title FROM news", 9, &tables)
            .unwrap()[0]
            .replacement,
        7..12
    );
    assert_eq!(
        engine
            .complete("SELECT \"Ré\" FROM news", 10, &tables)
            .unwrap_err()
            .to_string(),
        "Cursor 10 is not a UTF-8 boundary in the SQL text; use the editor's byte offset"
    );
}

fn catalog() -> Vec<Table> {
    [
        (
            "atlas",
            "benzinga",
            "news",
            &[
                ("id", "Int64"),
                ("title", "Utf8"),
                ("Résumé", "Utf8"),
                ("order", "Utf8"),
                ("quote\"name", "Utf8"),
                ("quote_other", "Utf8"),
            ][..],
        ),
        (
            "archive",
            "public",
            "orders",
            &[("id", "Int64"), ("total", "Float64")][..],
        ),
    ]
    .into_iter()
    .map(|(catalog, schema, name, columns)| Table {
        catalog: Some(catalog.to_owned()),
        database_schema: Some(schema.to_owned()),
        name: name.to_owned(),
        columns: columns
            .iter()
            .map(|(name, data_type)| Column {
                name: (*name).to_owned(),
                data_type: (*data_type).to_owned(),
            })
            .collect(),
    })
    .collect()
}
