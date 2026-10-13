//! QRIS (EMVCo merchant-presented QR) payloads for fictional merchants. Staging decodes them, but
//! paying one fails because the merchant isn't registered.

pub mod data;
pub mod tlv;

use anyhow::Result;
use image::RgbaImage;
use rand::Rng as _;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use crate::generator::{Generator, Rng};
use tlv::tlv;

pub const GUID: &str = "ID.CO.QRIS.WWW";
pub const NAME_MAX: usize = 25;
pub const CITY_MAX: usize = 15;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Tip {
    #[default]
    None,
    /// The payer's app asks for a tip.
    Prompt,
    Fixed(u64),
    Percent(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Merchant {
    pub name: String,
    pub city: String,
    pub postal: String,
    pub mcc: String,
    pub criteria: String,
    pub nmid: String,
    pub pan: String,
    pub mid: String,
    /// Dynamic codes carry a fixed amount; static ones let the payer type it.
    pub dynamic: bool,
    pub amount: Option<u64>,
    pub tip: Tip,
}

impl Default for Merchant {
    fn default() -> Self {
        Self {
            name: "QA KUCING STORE".into(),
            city: "BANDUNG".into(),
            postal: "40115".into(),
            mcc: "5814".into(),
            criteria: "UMI".into(),
            nmid: "ID1026000000001".into(),
            pan: "9360000812345678901".into(),
            mid: "000000000000001".into(),
            dynamic: false,
            amount: None,
            tip: Tip::None,
        }
    }
}

impl Merchant {
    pub fn random(rng: &mut Rng) -> Self {
        let (city, postal) = *data::CITIES.choose(rng).expect("cities");
        let pan_body = format!("936000{}", digits(rng, 12));
        let dynamic = rng.random_bool(0.5);
        Self {
            name: format!(
                "{} {} {}",
                data::NAME_FIRST.choose(rng).expect("names"),
                data::NAME_SECOND.choose(rng).expect("names"),
                digits(rng, 2)
            ),
            city: city.into(),
            postal: postal.into(),
            mcc: data::MCCS.choose(rng).expect("mccs").0.into(),
            criteria: "UMI".into(),
            nmid: format!("ID10{}", digits(rng, 11)),
            pan: format!("{pan_body}{}", luhn_digit(&pan_body)),
            mid: digits(rng, 15),
            dynamic,
            amount: dynamic.then(|| *data::RANDOM_AMOUNTS.choose(rng).expect("amounts")),
            tip: Tip::None,
        }
    }

    /// The payload string, CRC included.
    pub fn payload(&self) -> String {
        let mut s = tlv("00", "01") + &tlv("01", if self.dynamic { "12" } else { "11" });
        s += &tlv("26", &(tlv("00", GUID) + &tlv("01", &self.pan) + &tlv("02", &self.mid) + &tlv("03", &self.criteria)));
        s += &tlv("51", &(tlv("00", GUID) + &tlv("02", &self.nmid) + &tlv("03", &self.criteria)));
        s += &(tlv("52", &self.mcc) + &tlv("53", "360"));
        if let (true, Some(amount)) = (self.dynamic, self.amount) {
            s += &tlv("54", &amount.to_string());
        }
        match &self.tip {
            Tip::None => {}
            Tip::Prompt => s += &tlv("55", "01"),
            Tip::Fixed(value) => s += &(tlv("55", "02") + &tlv("56", &value.to_string())),
            Tip::Percent(value) => s += &(tlv("55", "03") + &tlv("57", value)),
        }
        s += &tlv("58", "ID");
        s += &tlv("59", &truncate(&self.name, NAME_MAX));
        s += &tlv("60", &truncate(&self.city, CITY_MAX));
        s += &tlv("61", &self.postal);
        s += "6304";
        let crc = tlv::crc16(&s);
        s + &crc
    }

    /// Reads the fields this generator writes back out of a payload, so an existing code can be edited.
    pub fn from_payload(payload: &str) -> Result<Self> {
        let fields = tlv::parse(payload)?;
        let mut m = Merchant { name: String::new(), city: String::new(), postal: String::new(), ..Default::default() };
        let mut tip_kind = None;
        for field in &fields {
            let child = |tag: &str| field.children.iter().find(|c| c.tag == tag).map(|c| c.value.clone());
            match field.tag.as_str() {
                "01" => m.dynamic = field.value == "12",
                "26" => {
                    m.pan = child("01").unwrap_or_default();
                    m.mid = child("02").unwrap_or_default();
                    m.criteria = child("03").unwrap_or(m.criteria);
                }
                "51" => m.nmid = child("02").unwrap_or_default(),
                "52" => m.mcc = field.value.clone(),
                "54" => m.amount = field.value.parse().ok(),
                "55" => tip_kind = Some(field.value.clone()),
                "56" => m.tip = Tip::Fixed(field.value.parse().unwrap_or_default()),
                "57" => m.tip = Tip::Percent(field.value.clone()),
                "59" => m.name = field.value.clone(),
                "60" => m.city = field.value.clone(),
                "61" => m.postal = field.value.clone(),
                _ => {}
            }
        }
        if tip_kind.as_deref() == Some("01") {
            m.tip = Tip::Prompt;
        }
        Ok(m)
    }
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn digits(rng: &mut Rng, count: usize) -> String {
    (0..count).map(|_| char::from(b'0' + rng.random_range(0..10u8))).collect()
}

/// The Luhn check digit for `body` (digits only).
pub fn luhn_digit(body: &str) -> char {
    let sum: u32 = body
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| {
            let d = u32::from(b - b'0');
            if i % 2 == 0 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();
    char::from(b'0' + ((10 - sum % 10) % 10) as u8)
}

pub fn luhn_valid(number: &str) -> bool {
    number.len() >= 2
        && number.bytes().all(|b| b.is_ascii_digit())
        && number[number.len() - 1..].starts_with(luhn_digit(&number[..number.len() - 1]))
}

pub struct Qris;

pub const QR_SCALE: u32 = 10;
pub const QR_MARGIN: u32 = 4;

impl Generator for Qris {
    const ID: &'static str = "qris";
    const NAME: &'static str = "QRIS";
    type Params = Merchant;

    fn randomize(rng: &mut Rng) -> Merchant {
        Merchant::random(rng)
    }

    fn generate(merchant: &Merchant, _seed: u64) -> Result<RgbaImage> {
        crate::qr::render(&merchant.payload(), QR_SCALE, QR_MARGIN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::rng;

    // Expected strings come from dg.ewallet/tools/qris_gen.py `build()`.
    #[test]
    fn matches_the_python_tool() {
        assert_eq!(
            Merchant::default().payload(),
            "00020101021126670014ID.CO.QRIS.WWW0119936000081234567890102150000000000000010303UMI51440014ID.CO.QRIS.WWW0215ID10260000000010303UMI5204581453033605802ID5915QA KUCING STORE6007BANDUNG610540115630499A1"
        );
        let dynamic = Merchant { dynamic: true, amount: Some(15000), ..Default::default() };
        assert_eq!(
            dynamic.payload(),
            "00020101021226670014ID.CO.QRIS.WWW0119936000081234567890102150000000000000010303UMI51440014ID.CO.QRIS.WWW0215ID10260000000010303UMI5204581453033605405150005802ID5915QA KUCING STORE6007BANDUNG610540115630411A7"
        );
        let custom = Merchant {
            name: "WARUNG OREN 07".into(),
            city: "SURABAYA".into(),
            nmid: "ID1012345678901".into(),
            mid: "123456789012345".into(),
            mcc: "5411".into(),
            postal: "60111".into(),
            dynamic: true,
            amount: Some(25000),
            ..Default::default()
        };
        assert_eq!(
            custom.payload(),
            "00020101021226670014ID.CO.QRIS.WWW0119936000081234567890102151234567890123450303UMI51440014ID.CO.QRIS.WWW0215ID10123456789010303UMI5204541153033605405250005802ID5914WARUNG OREN 076008SURABAYA6105601116304E012"
        );
    }

    #[test]
    fn payload_round_trips_through_parse() {
        for seed in 0..20 {
            let mut m = Merchant::random(&mut rng(seed));
            m.tip = match seed % 4 {
                0 => Tip::None,
                1 => Tip::Prompt,
                2 => Tip::Fixed(2000),
                _ => Tip::Percent("5".into()),
            };
            let payload = m.payload();
            assert_eq!(tlv::check_crc(&payload), tlv::CrcCheck::Valid);
            assert_eq!(Merchant::from_payload(&payload).unwrap(), m);
            assert!(luhn_valid(&m.pan), "{}", m.pan);
        }
    }

    #[test]
    fn long_names_are_cut_to_the_spec_limits() {
        let m = Merchant { name: "A".repeat(40), city: "B".repeat(20), ..Default::default() };
        let parsed = Merchant::from_payload(&m.payload()).unwrap();
        assert_eq!(parsed.name.len(), NAME_MAX);
        assert_eq!(parsed.city.len(), CITY_MAX);
    }

    #[test]
    fn luhn() {
        assert_eq!(luhn_digit("7992739871"), '3');
        assert!(luhn_valid("79927398713"));
        assert!(!luhn_valid("79927398710"));
    }

    #[test]
    fn generated_image_scans() {
        let m = Merchant::random(&mut rng(3));
        let image = Qris::generate(&m, 0).unwrap();
        assert_eq!(crate::qr::decode(&image::DynamicImage::ImageRgba8(image)).unwrap(), m.payload());
    }
}
