//! Identifiers and timestamps for test payloads.

use chrono::{DateTime, Local, TimeZone, Utc};
use rand::Rng as _;
use crate::generator::Rng;

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn uuid_v4(rng: &mut Rng) -> String {
    uuid::Builder::from_random_bytes(rng.random()).into_uuid().to_string()
}

/// Time-ordered UUID (v7) for `unix_ms`.
pub fn uuid_v7(rng: &mut Rng, unix_ms: u64) -> String {
    uuid::Builder::from_unix_timestamp_millis(unix_ms, &rng.random()).into_uuid().to_string()
}

/// ULID: 48-bit millisecond time then 80 random bits, Crockford base32.
pub fn ulid(rng: &mut Rng, unix_ms: u64) -> String {
    let value: u128 = (u128::from(unix_ms & 0xFFFF_FFFF_FFFF) << 80) | (rng.random::<u128>() & ((1u128 << 80) - 1));
    (0..26).rev().map(|i| char::from(CROCKFORD[((value >> (i * 5)) & 31) as usize])).collect()
}

/// The millisecond timestamp inside a ULID.
pub fn ulid_time(ulid: &str) -> Option<u64> {
    let ulid = ulid.trim().to_uppercase();
    if ulid.len() != 26 {
        return None;
    }
    let mut value: u128 = 0;
    for c in ulid.bytes() {
        value = (value << 5) | CROCKFORD.iter().position(|&d| d == c)? as u128;
    }
    Some((value >> 80) as u64)
}

pub fn now_ms() -> u64 {
    Utc::now().timestamp_millis().max(0) as u64
}

/// A number read as seconds or milliseconds (by magnitude), or an RFC 3339 / `YYYY-MM-DD HH:MM:SS` date.
pub fn parse_time(input: &str) -> Result<DateTime<Utc>, String> {
    let input = input.trim();
    if let Ok(number) = input.parse::<i64>() {
        let ms = if number.abs() >= 100_000_000_000 { number } else { number * 1000 };
        return Utc.timestamp_millis_opt(ms).single().ok_or_else(|| "out of range".into());
    }
    if let Ok(t) = DateTime::parse_from_rfc3339(input) {
        return Ok(t.with_timezone(&Utc));
    }
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%d-%m-%Y %H:%M:%S"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(input, format) {
            return Local.from_local_datetime(&naive).single().map(|t| t.with_timezone(&Utc)).ok_or_else(|| "ambiguous local time".into());
        }
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let naive = date.and_hms_opt(0, 0, 0).expect("midnight");
        return Local.from_local_datetime(&naive).single().map(|t| t.with_timezone(&Utc)).ok_or_else(|| "ambiguous local time".into());
    }
    Err("expected unix seconds/milliseconds or a date like 2026-10-13 14:30:00".into())
}

/// The same instant in the formats APIs commonly use.
pub fn time_formats(t: DateTime<Utc>) -> Vec<(&'static str, String)> {
    let local = t.with_timezone(&Local);
    let wib = t.with_timezone(&chrono::FixedOffset::east_opt(7 * 3600).expect("valid offset"));
    vec![
        ("Unix seconds", t.timestamp().to_string()),
        ("Unix milliseconds", t.timestamp_millis().to_string()),
        ("ISO 8601 UTC", t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        ("ISO 8601 WIB (+07:00)", wib.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)),
        ("Local", local.format("%Y-%m-%d %H:%M:%S %:z").to_string()),
        ("RFC 2822", t.to_rfc2822()),
        ("HTTP date", t.format("%a, %d %b %Y %H:%M:%S GMT").to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::rng;

    #[test]
    fn ulids_sort_by_time_and_decode() {
        let mut r = rng(1);
        let a = ulid(&mut r, 1_700_000_000_000);
        let b = ulid(&mut r, 1_700_000_000_001);
        assert_eq!(a.len(), 26);
        assert!(a < b);
        assert_eq!(ulid_time(&a), Some(1_700_000_000_000));
        assert_eq!(ulid_time("not a ulid"), None);
    }

    #[test]
    fn uuids_have_their_versions() {
        let mut r = rng(2);
        assert_eq!(uuid_v4(&mut r).chars().nth(14), Some('4'));
        let v7 = uuid_v7(&mut r, 1_700_000_000_000);
        assert_eq!(v7.chars().nth(14), Some('7'));
        assert!(v7.starts_with("018bcfe5"));
    }

    #[test]
    fn parses_seconds_milliseconds_and_dates() {
        assert_eq!(parse_time("1700000000").unwrap().timestamp(), 1_700_000_000);
        assert_eq!(parse_time("1700000000123").unwrap().timestamp_millis(), 1_700_000_000_123);
        assert_eq!(parse_time("2023-11-14T22:13:20Z").unwrap().timestamp(), 1_700_000_000);
        assert!(parse_time("2026-10-13 14:30:00").is_ok());
        assert!(parse_time("yesterday").is_err());
        let formats = time_formats(parse_time("1700000000").unwrap());
        assert_eq!(formats[2].1, "2023-11-14T22:13:20.000Z");
        assert_eq!(formats[3].1, "2023-11-15T05:13:20+07:00");
    }
}
