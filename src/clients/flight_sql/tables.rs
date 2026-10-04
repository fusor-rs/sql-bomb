use crate::{Error, Table};
use arrow_array::{Array, BinaryArray, RecordBatch, StringArray};
use arrow_ipc::convert::try_schema_from_ipc_buffer;

const CATALOG_NAME: &str = "catalog_name";
const DATABASE_SCHEMA_NAME: &str = "db_schema_name";
const TABLE_NAME: &str = "table_name";
const TABLE_TYPE: &str = "table_type";
const TABLE_SCHEMA: &str = "table_schema";

pub(super) fn append(batch: &RecordBatch, tables: &mut Vec<Table>) -> Result<(), Error> {
    let catalogs = column::<StringArray>(batch, CATALOG_NAME)?;
    let database_schemas = column::<StringArray>(batch, DATABASE_SCHEMA_NAME)?;
    let names = column::<StringArray>(batch, TABLE_NAME)?;
    let table_types = column::<StringArray>(batch, TABLE_TYPE)?;
    let schemas = column::<BinaryArray>(batch, TABLE_SCHEMA)?;
    for (name, column) in [
        (TABLE_NAME, names as &dyn Array),
        (TABLE_TYPE, table_types),
        (TABLE_SCHEMA, schemas),
    ] {
        if column.null_count() != 0 {
            return Err(Error::TableMetadata(name));
        }
    }
    for row in 0..batch.num_rows() {
        tables.push(Table {
            catalog: catalogs
                .is_valid(row)
                .then(|| catalogs.value(row).to_owned()),
            database_schema: database_schemas
                .is_valid(row)
                .then(|| database_schemas.value(row).to_owned()),
            name: names.value(row).to_owned(),
            table_type: table_types.value(row).to_owned(),
            schema: try_schema_from_ipc_buffer(schemas.value(row))?,
        });
    }
    Ok(())
}

fn column<'a, Column: Array + 'static>(
    batch: &'a RecordBatch,
    name: &'static str,
) -> Result<&'a Column, Error> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref())
        .ok_or(Error::TableMetadata(name))
}
