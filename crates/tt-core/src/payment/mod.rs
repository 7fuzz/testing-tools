//! Payment test data: Luhn-valid card numbers, bank account and virtual account numbers, and amount
//! edge cases. Card numbers are structurally valid only; they belong to no real account.

use chrono::{Datelike, NaiveDate};
use rand::Rng as _;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use crate::generator::Rng;
use crate::qris::{luhn_digit, luhn_valid};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Brand {
    Visa,
    Mastercard,
    Amex,
    Jcb,
    UnionPay,
    Discover,
}

impl Brand {
    pub const ALL: [Brand; 6] = [Brand::Visa, Brand::Mastercard, Brand::Amex, Brand::Jcb, Brand::UnionPay, Brand::Discover];

    pub fn label(self) -> &'static str {
        match self {
            Brand::Visa => "Visa",
            Brand::Mastercard => "Mastercard",
            Brand::Amex => "American Express",
            Brand::Jcb => "JCB",
            Brand::UnionPay => "UnionPay",
            Brand::Discover => "Discover",
        }
    }

    /// Prefix ranges (inclusive) and the card length.
    fn ranges(self) -> (&'static [(u64, u64)], usize) {
        match self {
            Brand::Visa => (&[(4, 4)], 16),
            Brand::Mastercard => (&[(51, 55), (2221, 2720)], 16),
            Brand::Amex => (&[(34, 34), (37, 37)], 15),
            Brand::Jcb => (&[(3528, 3589)], 16),
            Brand::UnionPay => (&[(62, 62)], 16),
            Brand::Discover => (&[(6011, 6011), (644, 649), (65, 65)], 16),
        }
    }

    pub fn cvv_len(self) -> usize {
        if self == Brand::Amex { 4 } else { 3 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub brand: Brand,
    pub number: String,
    pub expiry: String,
    pub cvv: String,
    pub holder: String,
}

fn digits(rng: &mut Rng, count: usize) -> String {
    (0..count).map(|_| char::from(b'0' + rng.random_range(0..10u8))).collect()
}

pub fn card_number(rng: &mut Rng, brand: Brand) -> String {
    let (ranges, len) = brand.ranges();
    let (lo, hi) = *ranges.choose(rng).expect("ranges");
    let prefix = rng.random_range(lo..=hi).to_string();
    let body = format!("{prefix}{}", digits(rng, len - 1 - prefix.len()));
    format!("{body}{}", luhn_digit(&body))
}

/// A card expiring 1–5 years after `today`.
pub fn card(rng: &mut Rng, brand: Brand, holder: &str, today: NaiveDate) -> Card {
    let year = today.year() + rng.random_range(1..=5);
    Card {
        brand,
        number: card_number(rng, brand),
        expiry: format!("{:02}/{:02}", rng.random_range(1..=12), year % 100),
        cvv: digits(rng, brand.cvv_len()),
        holder: holder.to_uppercase(),
    }
}

/// Brand by prefix and length, if the number fits one.
pub fn detect(number: &str) -> Option<Brand> {
    let d: String = number.chars().filter(char::is_ascii_digit).collect();
    Brand::ALL.into_iter().find(|brand| {
        let (ranges, len) = brand.ranges();
        d.len() == len
            && ranges.iter().any(|&(lo, hi)| {
                let width = lo.to_string().len();
                d.get(..width).and_then(|p| p.parse::<u64>().ok()).is_some_and(|p| (lo..=hi).contains(&p))
            })
    })
}

pub fn check_card(number: &str) -> Result<String, String> {
    let d: String = number.chars().filter(char::is_ascii_digit).collect();
    if d.len() < 12 {
        return Err("too short for a card number".into());
    }
    if !luhn_valid(&d) {
        return Err(format!("Luhn check fails; the last digit should be {}", luhn_digit(&d[..d.len() - 1])));
    }
    Ok(match detect(&d) {
        Some(brand) => format!("Luhn OK · {}", brand.label()),
        None => "Luhn OK · brand not recognised from the prefix".into(),
    })
}

pub fn group(number: &str) -> String {
    let d: Vec<char> = number.chars().filter(char::is_ascii_digit).collect();
    let sizes: &[usize] = if d.len() == 15 { &[4, 6, 5] } else { &[4, 4, 4, 4, 3] };
    let mut out = Vec::new();
    let mut i = 0;
    for &size in sizes {
        if i >= d.len() {
            break;
        }
        out.push(d[i..(i + size).min(d.len())].iter().collect::<String>());
        i += size;
    }
    out.join(" ")
}

pub struct Bank {
    pub name: &'static str,
    /// The 3-digit code used for interbank transfers (kode bank).
    pub code: &'static str,
    pub account_len: usize,
}

pub const BANKS: &[Bank] = &[
    Bank { name: "BCA", code: "014", account_len: 10 },
    Bank { name: "Mandiri", code: "008", account_len: 13 },
    Bank { name: "BNI", code: "009", account_len: 10 },
    Bank { name: "BRI", code: "002", account_len: 15 },
    Bank { name: "CIMB Niaga", code: "022", account_len: 13 },
    Bank { name: "Permata", code: "013", account_len: 10 },
    Bank { name: "Danamon", code: "011", account_len: 10 },
    Bank { name: "BSI", code: "451", account_len: 10 },
    Bank { name: "BTN", code: "200", account_len: 16 },
    Bank { name: "OCBC", code: "028", account_len: 12 },
];

pub fn account_number(rng: &mut Rng, bank: &Bank) -> String {
    format!("{}{}", rng.random_range(1..=9), digits(rng, bank.account_len - 1))
}

/// A virtual account: the merchant's company prefix, then a customer number padded to `length`.
pub fn virtual_account(rng: &mut Rng, prefix: &str, customer: Option<&str>, length: usize) -> String {
    let prefix: String = prefix.chars().filter(char::is_ascii_digit).collect();
    let room = length.saturating_sub(prefix.len());
    let customer: String = match customer {
        Some(c) => {
            let c: String = c.chars().filter(char::is_ascii_digit).collect();
            let c = c.trim_start_matches('0').to_string();
            let tail = c.len().saturating_sub(room);
            format!("{:0>room$}", &c[tail..])
        }
        None => digits(rng, room),
    };
    format!("{prefix}{customer}")
}

/// Amounts that tend to break validation, formatting or arithmetic, with why each is interesting.
pub const AMOUNT_EDGE_CASES: &[(&str, &str)] = &[
    ("0", "zero"),
    ("1", "smallest positive whole rupiah"),
    ("-1", "negative"),
    ("-0", "negative zero"),
    ("0.5", "fraction; rupiah has no cents"),
    ("1000.50", "decimal amount"),
    ("999", "just under a typical Rp1.000 minimum"),
    ("1000", "typical minimum"),
    ("9999", "just under Rp10.000"),
    ("10000", "common top-up minimum"),
    ("49999", "just under Rp50.000"),
    ("100000", "round"),
    ("999999", "just under a million"),
    ("1000000", "one million"),
    ("2000001", "just over Rp2.000.000 (a common unverified e-wallet limit)"),
    ("10000001", "just over Rp10.000.000 (a common verified e-wallet limit)"),
    ("2147483647", "i32 max"),
    ("2147483648", "i32 max + 1 (overflows 32-bit)"),
    ("9007199254740993", "2^53 + 1 (loses precision as a JS number)"),
    ("99999999999999999999", "wider than any 64-bit integer"),
    ("1e3", "scientific notation"),
    ("1.000", "Indonesian thousands separator (one thousand, or 1.0?)"),
    ("1,000", "English thousands separator"),
    ("Rp 10.000", "with currency symbol"),
    (" 1000 ", "padded with spaces"),
    ("1000abc", "trailing letters"),
    ("", "empty"),
    ("NaN", "not a number"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::rng;

    #[test]
    fn generated_cards_pass_luhn_and_detect_their_brand() {
        let mut r = rng(7);
        let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
        for brand in Brand::ALL {
            for _ in 0..50 {
                let c = card(&mut r, brand, "qa tester", today);
                assert!(luhn_valid(&c.number), "{}", c.number);
                assert_eq!(detect(&c.number), Some(brand), "{}", c.number);
                assert_eq!(c.cvv.len(), brand.cvv_len());
                assert!(check_card(&group(&c.number)).is_ok());
            }
        }
    }

    #[test]
    fn known_test_numbers() {
        assert_eq!(detect("4111111111111111"), Some(Brand::Visa));
        assert_eq!(detect("5555555555554444"), Some(Brand::Mastercard));
        assert_eq!(detect("378282246310005"), Some(Brand::Amex));
        assert!(check_card("4111111111111112").is_err());
        assert_eq!(group("378282246310005"), "3782 822463 10005");
    }

    #[test]
    fn accounts_and_virtual_accounts_have_the_right_length() {
        let mut r = rng(2);
        for bank in BANKS {
            assert_eq!(account_number(&mut r, bank).len(), bank.account_len);
        }
        assert_eq!(virtual_account(&mut r, "39358", Some("081234567890"), 16), "3935881234567890");
        assert_eq!(virtual_account(&mut r, "8808", None, 16).len(), 16);
        assert_eq!(virtual_account(&mut r, "123", Some("45"), 8), "12300045");
    }
}
