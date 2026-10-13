use super::columns::ColumnType;
use crate::{Error, Table};
use arrow_schema::{Field, Schema};
use tokio_postgres::{Row, types::Type};

pub(super) const QUERY: &str = include_str!("tables.sql");

const SCHEMA_NAME: &str = "schema_name";
const TABLE_NAME: &str = "table_name";
const TABLE_TYPE: &str = "table_type";
const COLUMN_NAMES: &str = "column_names";
const COLUMN_TYPES: &str = "column_types";
const NULLABLE: &str = "nullable";

pub(super) fn table(row: &Row) -> Result<Table, Error> {
    let names: Vec<&str> = row.try_get(COLUMN_NAMES)?;
    let types: Vec<u32> = row.try_get(COLUMN_TYPES)?;
    let nullable: Vec<bool> = row.try_get(NULLABLE)?;
    // Queries fail on columns of unsupported types, so discovery leaves them out.
    let fields: Vec<Field> = names
        .into_iter()
        .zip(types)
        .zip(nullable)
        .filter_map(|((name, oid), nullable)| {
            let column_type = ColumnType::new(&Type::from_oid(oid)?)?;
            Some(Field::new(name, column_type.data_type(), nullable))
        })
        .collect();
    Ok(Table {
        catalog: None,
        database_schema: Some(row.try_get(SCHEMA_NAME)?),
        name: row.try_get(TABLE_NAME)?,
        table_type: row.try_get(TABLE_TYPE)?,
        schema: Schema::new(fields),
    })
}
