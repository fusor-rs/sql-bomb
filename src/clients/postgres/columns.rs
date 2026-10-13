use super::values::{Days, Json, Microseconds, Numeric, Uuid};
use crate::Error;
use arrow_array::{
    ArrayRef, BinaryArray, BooleanArray, Date32Array, Float32Array, Float64Array, Int16Array,
    Int32Array, Int64Array, RecordBatch, StringArray, Time64MicrosecondArray,
    TimestampMicrosecondArray, UInt32Array,
};
use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use std::sync::Arc;
use tokio_postgres::{
    Column, Row,
    types::{FromSql, Type},
};

const UTC: &str = "+00:00";

pub(super) struct Columns {
    pub(super) schema: SchemaRef,
    types: Vec<ColumnType>,
}

#[derive(Clone, Copy)]
pub(super) enum ColumnType {
    Boolean,
    Int16,
    Int32,
    Int64,
    Oid,
    Float32,
    Float64,
    Text,
    Bytes,
    Numeric,
    Uuid,
    Json,
    Date,
    Time,
    Timestamp,
    TimestampTz,
}

impl Columns {
    pub(super) fn new(columns: &[Column]) -> Result<Self, Error> {
        let mut fields = Vec::with_capacity(columns.len());
        let mut types = Vec::with_capacity(columns.len());
        for column in columns {
            let column_type =
                ColumnType::new(column.type_()).ok_or_else(|| Error::PostgresType {
                    column: column.name().to_owned(),
                    type_name: column.type_().name().to_owned(),
                })?;
            fields.push(Field::new(column.name(), column_type.data_type(), true));
            types.push(column_type);
        }
        Ok(Self {
            schema: Arc::new(Schema::new(fields)),
            types,
        })
    }

    pub(super) fn batch(&self, rows: &[Row]) -> Result<RecordBatch, Error> {
        let arrays = self
            .types
            .iter()
            .enumerate()
            .map(|(index, column_type)| column_type.array(rows, index))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RecordBatch::try_new(self.schema.clone(), arrays)?)
    }
}

impl ColumnType {
    pub(super) fn new(postgres_type: &Type) -> Option<Self> {
        Some(match *postgres_type {
            Type::BOOL => Self::Boolean,
            Type::INT2 => Self::Int16,
            Type::INT4 => Self::Int32,
            Type::INT8 => Self::Int64,
            Type::OID => Self::Oid,
            Type::FLOAT4 => Self::Float32,
            Type::FLOAT8 => Self::Float64,
            Type::BYTEA => Self::Bytes,
            Type::NUMERIC => Self::Numeric,
            Type::UUID => Self::Uuid,
            Type::JSON | Type::JSONB => Self::Json,
            Type::DATE => Self::Date,
            Type::TIME => Self::Time,
            Type::TIMESTAMP => Self::Timestamp,
            Type::TIMESTAMPTZ => Self::TimestampTz,
            ref text if <&str as FromSql>::accepts(text) => Self::Text,
            _ => return None,
        })
    }

    pub(super) fn data_type(self) -> DataType {
        match self {
            Self::Boolean => DataType::Boolean,
            Self::Int16 => DataType::Int16,
            Self::Int32 => DataType::Int32,
            Self::Int64 => DataType::Int64,
            Self::Oid => DataType::UInt32,
            Self::Float32 => DataType::Float32,
            Self::Float64 => DataType::Float64,
            Self::Text | Self::Numeric | Self::Uuid | Self::Json => DataType::Utf8,
            Self::Bytes => DataType::Binary,
            Self::Date => DataType::Date32,
            Self::Time => DataType::Time64(TimeUnit::Microsecond),
            Self::Timestamp => DataType::Timestamp(TimeUnit::Microsecond, None),
            Self::TimestampTz => DataType::Timestamp(TimeUnit::Microsecond, Some(UTC.into())),
        }
    }

    fn array(self, rows: &[Row], index: usize) -> Result<ArrayRef, tokio_postgres::Error> {
        Ok(match self {
            Self::Boolean => Arc::new(values::<BooleanArray, bool, bool>(rows, index)?),
            Self::Int16 => Arc::new(values::<Int16Array, i16, i16>(rows, index)?),
            Self::Int32 => Arc::new(values::<Int32Array, i32, i32>(rows, index)?),
            Self::Int64 => Arc::new(values::<Int64Array, i64, i64>(rows, index)?),
            Self::Oid => Arc::new(values::<UInt32Array, u32, u32>(rows, index)?),
            Self::Float32 => Arc::new(values::<Float32Array, f32, f32>(rows, index)?),
            Self::Float64 => Arc::new(values::<Float64Array, f64, f64>(rows, index)?),
            Self::Text => Arc::new(values::<StringArray, &str, &str>(rows, index)?),
            Self::Bytes => Arc::new(values::<BinaryArray, &[u8], &[u8]>(rows, index)?),
            Self::Numeric => Arc::new(values::<StringArray, Numeric, String>(rows, index)?),
            Self::Uuid => Arc::new(values::<StringArray, Uuid, String>(rows, index)?),
            Self::Json => Arc::new(values::<StringArray, Json, &str>(rows, index)?),
            Self::Date => Arc::new(values::<Date32Array, Days, i32>(rows, index)?),
            Self::Time => Arc::new(values::<Time64MicrosecondArray, Microseconds, i64>(
                rows, index,
            )?),
            Self::Timestamp => Arc::new(timestamps(rows, index)?),
            Self::TimestampTz => Arc::new(timestamps(rows, index)?.with_timezone(UTC)),
        })
    }
}

fn timestamps(
    rows: &[Row],
    index: usize,
) -> Result<TimestampMicrosecondArray, tokio_postgres::Error> {
    values::<_, Microseconds, i64>(rows, index)
}

fn values<'a, Output, Value, Element>(
    rows: &'a [Row],
    index: usize,
) -> Result<Output, tokio_postgres::Error>
where
    Output: FromIterator<Option<Element>>,
    Value: FromSql<'a> + Into<Element>,
{
    rows.iter()
        .map(|row| Ok(row.try_get::<_, Option<Value>>(index)?.map(Into::into)))
        .collect()
}
