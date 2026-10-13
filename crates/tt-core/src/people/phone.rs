//! Indonesian mobile numbers by operator prefix.

use rand::Rng as _;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use crate::generator::Rng;

pub const OPERATORS: &[(&str, &[&str])] = &[
    ("Telkomsel", &["0811", "0812", "0813", "0821", "0822", "0823", "0851", "0852", "0853"]),
    ("Indosat", &["0814", "0815", "0816", "0855", "0856", "0857", "0858"]),
    ("XL", &["0817", "0818", "0819", "0859", "0877", "0878"]),
    ("Axis", &["0831", "0832", "0833", "0838"]),
    ("Tri", &["0895", "0896", "0897", "0898", "0899"]),
    ("Smartfren", &["0881", "0882", "0883", "0884", "0885", "0886", "0887", "0888", "0889"]),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    /// 0812…
    #[default]
    Local,
    /// +62812…
    International,
    /// 62812…
    Msisdn,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Local, Format::International, Format::Msisdn];

    pub fn label(self) -> &'static str {
        match self {
            Format::Local => "08…",
            Format::International => "+628…",
            Format::Msisdn => "628…",
        }
    }
}

pub fn format(local: &str, format: Format) -> String {
    let rest = local.trim_start_matches('0');
    match format {
        Format::Local => local.to_string(),
        Format::International => format!("+62{rest}"),
        Format::Msisdn => format!("62{rest}"),
    }
}

/// A number of 11–13 digits (local form) on `operator`, or any operator.
pub fn random(rng: &mut Rng, operator: Option<&str>, style: Format) -> (String, &'static str) {
    let (name, prefixes) = operator
        .and_then(|o| OPERATORS.iter().find(|(n, _)| n.eq_ignore_ascii_case(o)))
        .copied()
        .unwrap_or_else(|| *OPERATORS.choose(rng).expect("operators"));
    let prefix = prefixes.choose(rng).expect("prefixes");
    let rest: String = (0..rng.random_range(7..=9)).map(|_| char::from(b'0' + rng.random_range(0..10u8))).collect();
    (format(&format!("{prefix}{rest}"), style), name)
}

/// Normalises any of the three forms to the local `08…` form.
pub fn to_local(number: &str) -> Option<String> {
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    let local = if let Some(rest) = digits.strip_prefix("62") { format!("0{rest}") } else { digits };
    local.starts_with("08").then_some(local)
}

pub fn operator_of(number: &str) -> Option<&'static str> {
    let local = to_local(number)?;
    if !(10..=13).contains(&local.len()) {
        return None;
    }
    OPERATORS.iter().find(|(_, prefixes)| prefixes.iter().any(|p| local.starts_with(p))).map(|(n, _)| *n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::rng;

    #[test]
    fn numbers_round_trip_through_every_format() {
        let mut r = rng(3);
        for style in Format::ALL {
            for _ in 0..50 {
                let (number, operator) = random(&mut r, None, style);
                assert_eq!(operator_of(&number), Some(operator), "{number}");
            }
        }
        let (number, operator) = random(&mut r, Some("tri"), Format::International);
        assert_eq!(operator, "Tri");
        assert!(number.starts_with("+6289"));
    }

    #[test]
    fn rejects_landlines_and_bad_lengths() {
        assert_eq!(operator_of("0215551234"), None);
        assert_eq!(operator_of("0812"), None);
        assert_eq!(operator_of("+62 812-3456-7890"), Some("Telkomsel"));
    }
}
