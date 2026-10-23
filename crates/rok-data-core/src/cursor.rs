use std::fmt;
use std::str::FromStr;

use crate::{Error, Value};

/// An opaque position in a keyset-paginated result, returned by
/// [`Select::cursor_paginate`](crate::Select::cursor_paginate).
///
/// A cursor holds the ordering values of a page's edge row and which way
/// it points: a page's `next` cursor points after its last row, its
/// `prev` cursor before its first. It round-trips through a compact,
/// URL-safe string (`to_string()` / `parse()`), so it can be handed to API
/// clients as a "next page" / "previous page" token. Treat the string as
/// opaque: its format may change between minor versions.
#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    pub(crate) values: Vec<Value>,
    pub(crate) backward: bool,
}

impl Cursor {
    /// A cursor positioned after a row with these ordering values (one per
    /// `ORDER BY` column, in order, including the primary key tiebreaker):
    /// it continues forward.
    pub fn new(values: impl IntoIterator<Item = Value>) -> Self {
        Self {
            values: values.into_iter().collect(),
            backward: false,
        }
    }

    /// A cursor positioned before a row with these ordering values: it
    /// pages backward, to the rows just before that one.
    pub fn before(values: impl IntoIterator<Item = Value>) -> Self {
        Self {
            values: values.into_iter().collect(),
            backward: true,
        }
    }

    /// The ordering values of the row this cursor points from.
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// `true` for a cursor that pages backward (a page's `prev`).
    pub fn is_backward(&self) -> bool {
        self.backward
    }
}

/// One page of a keyset-paginated query: the rows, and the cursors to the
/// pages on either side.
#[derive(Debug, Clone, PartialEq)]
pub struct CursorPage<T> {
    /// Records on this page.
    pub items: Vec<T>,
    /// Cursor for the following page, or `None` on the last page.
    pub next: Option<Cursor>,
    /// Cursor for the preceding page, or `None` on the first page.
    pub prev: Option<Cursor>,
    /// Maximum number of records per page.
    pub per_page: u64,
}

impl<T> CursorPage<T> {
    /// `true` if a later page exists.
    pub fn has_next(&self) -> bool {
        self.next.is_some()
    }

    /// `true` if a later page exists — Laravel's `hasMorePages`.
    pub fn has_more(&self) -> bool {
        self.has_next()
    }

    /// `true` if an earlier page exists.
    pub fn has_prev(&self) -> bool {
        self.prev.is_some()
    }

    /// The cursor for the following page — Laravel's `nextCursor`.
    pub fn next_cursor(&self) -> Option<&Cursor> {
        self.next.as_ref()
    }

    /// The cursor for the preceding page — Laravel's `previousCursor`.
    pub fn prev_cursor(&self) -> Option<&Cursor> {
        self.prev.as_ref()
    }

    /// Transform every record, keeping the cursors.
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> CursorPage<U> {
        CursorPage {
            items: self.items.into_iter().map(f).collect(),
            next: self.next,
            prev: self.prev,
            per_page: self.per_page,
        }
    }
}

impl<T> IntoIterator for CursorPage<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

// ----- encoding --------------------------------------------------------------
//
// `tag` + `~` (NULL) or `tag` + hex(payload), joined with `.`; every character
// is URL-safe.

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Result<Vec<u8>, Error> {
    if !s.len().is_multiple_of(2) {
        return Err(Error::InvalidCursor("odd hex length".into()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| Error::InvalidCursor("bad hex digit".into()))
        })
        .collect()
}

fn text(bytes: Vec<u8>) -> Result<String, Error> {
    String::from_utf8(bytes).map_err(|_| Error::InvalidCursor("invalid utf-8".into()))
}

fn parse<T: FromStr>(bytes: Vec<u8>) -> Result<T, Error> {
    text(bytes)?
        .parse()
        .map_err(|_| Error::InvalidCursor("malformed value".into()))
}

/// Parse `months,days,microseconds` (the cursor form of an `INTERVAL`).
fn interval(s: &str) -> Result<sqlx::postgres::types::PgInterval, Error> {
    let malformed = || Error::InvalidCursor("malformed interval".into());
    let mut parts = s.split(',');
    let mut next = || parts.next().ok_or_else(malformed);
    let interval = sqlx::postgres::types::PgInterval {
        months: next()?.parse().map_err(|_| malformed())?,
        days: next()?.parse().map_err(|_| malformed())?,
        microseconds: next()?.parse().map_err(|_| malformed())?,
    };
    if parts.next().is_some() {
        return Err(malformed());
    }
    Ok(interval)
}

fn encode(value: &Value) -> String {
    fn part<T>(tag: char, v: &Option<T>, f: impl FnOnce(&T) -> Vec<u8>) -> String {
        match v {
            Some(v) => format!("{tag}{}", hex(&f(v))),
            None => format!("{tag}~"),
        }
    }
    let s = |v: &dyn ToString| v.to_string().into_bytes();
    match value {
        Value::Bool(v) => part('b', v, |v| s(v)),
        Value::I16(v) => part('h', v, |v| s(v)),
        Value::I32(v) => part('i', v, |v| s(v)),
        Value::I64(v) => part('l', v, |v| s(v)),
        Value::F32(v) => part('f', v, |v| s(&v.to_bits())),
        Value::F64(v) => part('d', v, |v| s(&v.to_bits())),
        Value::String(v) => part('s', v, |v| v.as_bytes().to_vec()),
        Value::Bytes(v) => part('y', v, |v| v.clone()),
        #[cfg(feature = "uuid")]
        Value::Uuid(v) => part('u', v, |v| s(v)),
        #[cfg(feature = "chrono")]
        Value::DateTime(v) => part('t', v, |v| v.to_rfc3339().into_bytes()),
        #[cfg(feature = "chrono")]
        Value::NaiveDateTime(v) => part('n', v, |v| s(v)),
        #[cfg(feature = "chrono")]
        Value::NaiveDate(v) => part('D', v, |v| s(v)),
        #[cfg(feature = "chrono")]
        Value::NaiveTime(v) => part('T', v, |v| s(v)),
        #[cfg(feature = "json")]
        Value::Json(v) => part('j', v, |v| s(v)),
        #[cfg(feature = "decimal")]
        Value::Decimal(v) => part('m', v, |v| s(v)),
        Value::Interval(v) => part('v', v, |v| {
            format!("{},{},{}", v.months, v.days, v.microseconds).into_bytes()
        }),
        // Custom types can't be decoded generically; `x` makes the cursor
        // fail to parse with a clear error instead of misbehaving.
        Value::Custom(v) => format!("x{}", hex(format!("{v:?}").as_bytes())),
    }
}

fn decode(part: &str) -> Result<Value, Error> {
    let mut chars = part.chars();
    let tag = chars
        .next()
        .ok_or_else(|| Error::InvalidCursor("empty value".into()))?;
    let rest = chars.as_str();
    let payload = if rest == "~" {
        None
    } else {
        Some(unhex(rest)?)
    };
    macro_rules! val {
        ($variant:ident, |$b:ident| $conv:expr) => {
            Value::$variant(match payload {
                Some($b) => Some($conv),
                None => None,
            })
        };
    }
    Ok(match tag {
        'b' => val!(Bool, |b| parse(b)?),
        'h' => val!(I16, |b| parse(b)?),
        'i' => val!(I32, |b| parse(b)?),
        'l' => val!(I64, |b| parse(b)?),
        'f' => val!(F32, |b| f32::from_bits(parse(b)?)),
        'd' => val!(F64, |b| f64::from_bits(parse(b)?)),
        's' => val!(String, |b| text(b)?),
        'y' => val!(Bytes, |b| b),
        #[cfg(feature = "uuid")]
        'u' => val!(Uuid, |b| parse(b)?),
        #[cfg(feature = "chrono")]
        't' => val!(DateTime, |b| {
            sqlx::types::chrono::DateTime::parse_from_rfc3339(&text(b)?)
                .map_err(|_| Error::InvalidCursor("malformed timestamp".into()))?
                .with_timezone(&sqlx::types::chrono::Utc)
        }),
        #[cfg(feature = "chrono")]
        'n' => val!(NaiveDateTime, |b| parse(b)?),
        #[cfg(feature = "chrono")]
        'D' => val!(NaiveDate, |b| parse(b)?),
        #[cfg(feature = "chrono")]
        'T' => val!(NaiveTime, |b| parse(b)?),
        #[cfg(feature = "json")]
        'j' => val!(Json, |b| serde_json::from_slice(&b)
            .map_err(|_| Error::InvalidCursor("malformed json".into()))?),
        #[cfg(feature = "decimal")]
        'm' => val!(Decimal, |b| parse(b)?),
        'v' => val!(Interval, |b| interval(&text(b)?)?),
        'x' => {
            return Err(Error::InvalidCursor(
                "custom column types can't be used as keyset pagination columns".into(),
            ));
        }
        other => return Err(Error::InvalidCursor(format!("unknown type tag `{other}`"))),
    })
}

/// The mark of a backward cursor's string: never a type tag, URL-safe.
/// Forward cursors carry no mark, so 0.2 tokens parse unchanged.
const BACKWARD: char = '-';

impl fmt::Display for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.backward {
            write!(f, "{BACKWARD}")?;
        }
        let parts: Vec<String> = self.values.iter().map(encode).collect();
        f.write_str(&parts.join("."))
    }
}

impl FromStr for Cursor {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let (backward, s) = match s.strip_prefix(BACKWARD) {
            Some(rest) => (true, rest),
            None => (false, s),
        };
        if s.is_empty() {
            return Err(Error::InvalidCursor("empty cursor".into()));
        }
        Ok(Self {
            values: s.split('.').map(decode).collect::<Result<_, _>>()?,
            backward,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let cursor = Cursor::new([
            Value::from(42_i64),
            Value::from("a.b~c/é"),
            Value::from(None::<i32>),
            Value::from(-1.5_f64),
            Value::from(true),
            Value::from(vec![0_u8, 255]),
            Value::from(sqlx::postgres::types::PgInterval {
                months: 1,
                days: -2,
                microseconds: 3_000_000,
            }),
            Value::from(None::<sqlx::postgres::types::PgInterval>),
        ]);
        let encoded = cursor.to_string();
        assert!(
            encoded
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".~".contains(c)),
            "{encoded}"
        );
        assert_eq!(encoded.parse::<Cursor>().unwrap(), cursor);
    }

    #[cfg(feature = "decimal")]
    #[test]
    fn round_trips_decimals() {
        let price = sqlx::types::Decimal::new(-123_456, 3);
        let cursor = Cursor::new([
            Value::from(price),
            Value::from(None::<sqlx::types::Decimal>),
        ]);
        assert_eq!(cursor.to_string().parse::<Cursor>().unwrap(), cursor);
    }

    #[test]
    fn a_backward_cursor_keeps_its_direction_and_a_forward_token_is_unchanged() {
        let forward = Cursor::new([Value::from(42_i64)]);
        let backward = Cursor::before([Value::from(42_i64)]);
        // The forward form is the 0.2 token, byte for byte.
        assert_eq!(forward.to_string(), "l3432");
        assert_eq!(backward.to_string(), "-l3432");
        let parsed: Cursor = backward.to_string().parse().unwrap();
        assert_eq!(parsed, backward);
        assert!(parsed.is_backward() && !forward.is_backward());
        assert!("-".parse::<Cursor>().is_err(), "a mark with no values");
    }

    #[test]
    fn rejects_garbage() {
        // `v` + hex("1,2") and hex("1,2,3,4"): intervals need exactly three parts.
        for bad in ["", "x12", "lzz", "l1", "s~.q", "v312c32", "v312c322c332c34"] {
            assert!(bad.parse::<Cursor>().is_err(), "{bad}");
        }
    }
}
