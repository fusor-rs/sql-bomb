use std::{error::Error, fmt::Write};
use tokio_postgres::types::{FromSql, Type};

type DecodeError = Box<dyn Error + Sync + Send>;

const POSTGRES_EPOCH_DAYS: i32 = 10_957;
const MICROSECONDS_PER_DAY: i64 = 86_400_000_000;
const POSTGRES_EPOCH_MICROSECONDS: i64 = POSTGRES_EPOCH_DAYS as i64 * MICROSECONDS_PER_DAY;
const OUT_OF_RANGE: &str =
    "infinite or out-of-range value has no Arrow equivalent; cast it to text";
const JSONB_VERSION: u8 = 1;
const UNKNOWN_JSONB: &str = "unsupported jsonb format version; cast the value to text";
const UUID_GROUP_STARTS: [usize; 4] = [4, 6, 8, 10];
const UUID_BYTES: usize = 16;
const NUMERIC_WORD_BYTES: usize = 2;
const NUMERIC_WORD_DIGITS: usize = 4;
const NUMERIC_NEGATIVE: u16 = 0x4000;
const NUMERIC_NAN: u16 = 0xC000;
const NUMERIC_INFINITY: u16 = 0xD000;
const NUMERIC_NEGATIVE_INFINITY: u16 = 0xF000;
const MALFORMED_NUMERIC: &str = "malformed numeric value";

// Days since the Unix epoch.
pub(super) struct Days(i32);

// Microseconds since midnight for times, or since the Unix epoch for timestamps.
pub(super) struct Microseconds(i64);

pub(super) struct Numeric(String);

pub(super) struct Uuid(String);

pub(super) struct Json<'a>(&'a str);

impl From<Days> for i32 {
    fn from(days: Days) -> Self {
        days.0
    }
}

impl From<Microseconds> for i64 {
    fn from(microseconds: Microseconds) -> Self {
        microseconds.0
    }
}

impl From<Numeric> for String {
    fn from(numeric: Numeric) -> Self {
        numeric.0
    }
}

impl From<Uuid> for String {
    fn from(uuid: Uuid) -> Self {
        uuid.0
    }
}

impl<'a> From<Json<'a>> for &'a str {
    fn from(json: Json<'a>) -> Self {
        json.0
    }
}

impl FromSql<'_> for Days {
    fn from_sql(_: &Type, raw: &[u8]) -> Result<Self, DecodeError> {
        match i32::from_be_bytes(raw.try_into()?) {
            i32::MIN | i32::MAX => Err(OUT_OF_RANGE.into()),
            days => Ok(Self(
                days.checked_add(POSTGRES_EPOCH_DAYS).ok_or(OUT_OF_RANGE)?,
            )),
        }
    }

    fn accepts(postgres_type: &Type) -> bool {
        *postgres_type == Type::DATE
    }
}

impl FromSql<'_> for Microseconds {
    fn from_sql(postgres_type: &Type, raw: &[u8]) -> Result<Self, DecodeError> {
        let microseconds = i64::from_be_bytes(raw.try_into()?);
        if *postgres_type == Type::TIME {
            return Ok(Self(microseconds));
        }
        match microseconds {
            i64::MIN | i64::MAX => Err(OUT_OF_RANGE.into()),
            since_postgres_epoch => Ok(Self(
                since_postgres_epoch
                    .checked_add(POSTGRES_EPOCH_MICROSECONDS)
                    .ok_or(OUT_OF_RANGE)?,
            )),
        }
    }

    fn accepts(postgres_type: &Type) -> bool {
        matches!(
            *postgres_type,
            Type::TIME | Type::TIMESTAMP | Type::TIMESTAMPTZ
        )
    }
}

impl FromSql<'_> for Numeric {
    fn from_sql(_: &Type, raw: &[u8]) -> Result<Self, DecodeError> {
        let (words, []) = raw.as_chunks::<NUMERIC_WORD_BYTES>() else {
            return Err(MALFORMED_NUMERIC.into());
        };
        let words: Vec<u16> = words.iter().map(|word| u16::from_be_bytes(*word)).collect();
        let [count, weight, sign, scale, digits @ ..] = words.as_slice() else {
            return Err(MALFORMED_NUMERIC.into());
        };
        if digits.len() != usize::from(*count) {
            return Err(MALFORMED_NUMERIC.into());
        }
        let mut text = match *sign {
            NUMERIC_NAN => return Ok(Self("NaN".into())),
            NUMERIC_INFINITY => return Ok(Self("Infinity".into())),
            NUMERIC_NEGATIVE_INFINITY => return Ok(Self("-Infinity".into())),
            NUMERIC_NEGATIVE => String::from("-"),
            _ => String::new(),
        };
        let weight = i32::from(weight.cast_signed());
        // Postgres omits zero digit words before and after the stored ones.
        let digit = |position: i32| {
            usize::try_from(position)
                .ok()
                .and_then(|index| digits.get(index))
                .map_or(0, |digit| *digit)
        };
        if weight < 0 {
            text.push('0');
        } else {
            write!(text, "{}", digit(0))?;
            for position in 1..=weight {
                write!(text, "{:0NUMERIC_WORD_DIGITS$}", digit(position))?;
            }
        }
        let scale = usize::from(*scale);
        if scale > 0 {
            text.push('.');
            let fraction_end = text.len() + scale;
            let mut position = weight + 1;
            while text.len() < fraction_end {
                write!(text, "{:0NUMERIC_WORD_DIGITS$}", digit(position))?;
                position += 1;
            }
            text.truncate(fraction_end);
        }
        Ok(Self(text))
    }

    fn accepts(postgres_type: &Type) -> bool {
        *postgres_type == Type::NUMERIC
    }
}

impl FromSql<'_> for Uuid {
    fn from_sql(_: &Type, raw: &[u8]) -> Result<Self, DecodeError> {
        let bytes: [u8; UUID_BYTES] = raw.try_into()?;
        let mut text = String::new();
        for (index, byte) in bytes.iter().enumerate() {
            if UUID_GROUP_STARTS.contains(&index) {
                text.push('-');
            }
            write!(text, "{byte:02x}")?;
        }
        Ok(Self(text))
    }

    fn accepts(postgres_type: &Type) -> bool {
        *postgres_type == Type::UUID
    }
}

impl<'a> FromSql<'a> for Json<'a> {
    fn from_sql(postgres_type: &Type, raw: &'a [u8]) -> Result<Self, DecodeError> {
        let text = if *postgres_type == Type::JSONB {
            match raw.split_first() {
                Some((&JSONB_VERSION, text)) => text,
                _ => return Err(UNKNOWN_JSONB.into()),
            }
        } else {
            raw
        };
        Ok(Self(std::str::from_utf8(text)?))
    }

    fn accepts(postgres_type: &Type) -> bool {
        matches!(*postgres_type, Type::JSON | Type::JSONB)
    }
}
