//! NPWP (taxpayer number). Since 2024 individuals use their 16-digit NIK as NPWP; the old 15-digit
//! form is `XX.XXX.XXX.C-KKK.BBB`: tax type, serial, check digit, tax office (KPP), branch.
//! The check digit here is the Luhn digit of the first eight, which is what common validators use;
//! DJP doesn't publish the algorithm, so treat it as a format check, not proof of validity.

use rand::Rng as _;
use crate::generator::Rng;
use crate::qris::{luhn_digit, luhn_valid};

/// NPWP 16 for an individual: the NIK itself.
pub fn from_nik(nik: &str) -> String {
    nik.to_string()
}

/// A random 15-digit NPWP (individual tax types 04–09) with a Luhn check digit.
pub fn random15(rng: &mut Rng) -> String {
    let body = format!("{:02}{:06}", rng.random_range(4..=9), rng.random_range(0..1_000_000));
    let kpp = rng.random_range(1..=999);
    let branch = if rng.random_bool(0.85) { 0 } else { rng.random_range(1..=9) };
    format!("{body}{}{kpp:03}{branch:03}", luhn_digit(&body))
}

/// The 16-digit form for entities: a leading 0 plus the 15 digits.
pub fn to16(npwp15: &str) -> String {
    format!("0{}", digits(npwp15))
}

pub fn format15(npwp15: &str) -> String {
    let d = digits(npwp15);
    if d.len() != 15 {
        return npwp15.to_string();
    }
    format!("{}.{}.{}.{}-{}.{}", &d[0..2], &d[2..5], &d[5..8], &d[8..9], &d[9..12], &d[12..15])
}

fn digits(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

/// Accepts 15 digits (dots and dash optional) or 16 digits.
pub fn validate(text: &str) -> Result<&'static str, String> {
    let d = digits(text);
    match d.len() {
        15 => {
            if luhn_valid(&d[..9]) {
                Ok("NPWP 15, check digit OK")
            } else {
                Err(format!("check digit should be {}", luhn_digit(&d[..8])))
            }
        }
        16 if d.starts_with('0') => {
            if luhn_valid(&d[1..10]) { Ok("NPWP 16 (entity), check digit OK") } else { Err("check digit of the 15-digit part is wrong".into()) }
        }
        16 => crate::ktp::identity::parse_nik(&d).map(|_| "NPWP 16 (individual, same as NIK)").map_err(|e| format!("as a NIK: {e}")),
        n => Err(format!("{n} digits; NPWP has 15 or 16")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::rng;

    #[test]
    fn random_npwp_validate_in_both_forms() {
        let mut r = rng(4);
        for _ in 0..100 {
            let n = random15(&mut r);
            assert_eq!(n.len(), 15);
            assert!(validate(&n).is_ok(), "{n}");
            assert!(validate(&format15(&n)).is_ok());
            assert!(validate(&to16(&n)).is_ok());
        }
    }

    #[test]
    fn rejects_bad_check_digits_and_lengths() {
        let n = random15(&mut rng(1));
        let mut broken: Vec<char> = n.chars().collect();
        broken[8] = if broken[8] == '0' { '1' } else { '0' };
        assert!(validate(&broken.iter().collect::<String>()).is_err());
        assert!(validate("123").is_err());
        assert!(validate("3273015506900001").is_ok());
    }
}
