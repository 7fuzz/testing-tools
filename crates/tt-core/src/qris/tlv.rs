use anyhow::{Result, bail};

/// One EMVCo tag-length-value field; templates (26–51, 62, 64) carry their sub-fields in `children`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub tag: String,
    pub value: String,
    pub children: Vec<Field>,
}

// Lengths count characters, matching the Python and JS tools; QRIS values are ASCII in practice.
pub fn tlv(tag: &str, value: &str) -> String {
    format!("{tag}{:02}{value}", value.chars().count())
}

/// CRC-16/CCITT-FALSE (init 0xFFFF, poly 0x1021) as four uppercase hex digits, the format tag 63 holds.
pub fn crc16(data: &str) -> String {
    let mut crc: u16 = 0xFFFF;
    for byte in data.bytes() {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    format!("{crc:04X}")
}

pub fn is_template(tag: &str) -> bool {
    matches!(tag.parse::<u8>(), Ok(26..=51 | 62 | 64))
}

pub fn parse(payload: &str) -> Result<Vec<Field>> {
    parse_level(payload, true)
}

fn parse_level(text: &str, top: bool) -> Result<Vec<Field>> {
    let chars: Vec<char> = text.chars().collect();
    let mut fields = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if i + 4 > chars.len() {
            bail!("truncated field at position {i}");
        }
        let tag: String = chars[i..i + 2].iter().collect();
        let len_text: String = chars[i + 2..i + 4].iter().collect();
        if !tag.chars().all(|c| c.is_ascii_digit()) || !len_text.chars().all(|c| c.is_ascii_digit()) {
            bail!("bad tag or length at position {i}");
        }
        let len: usize = len_text.parse()?;
        let start = i + 4;
        if start + len > chars.len() {
            bail!("tag {tag} declares length {len} but only {} characters remain", chars.len() - start);
        }
        let value: String = chars[start..start + len].iter().collect();
        let children = if top && is_template(&tag) { parse_level(&value, false)? } else { Vec::new() };
        fields.push(Field { tag, value, children });
        i = start + len;
    }
    Ok(fields)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrcCheck {
    Valid,
    Invalid { expected: String, found: String },
    Missing,
}

/// Checks the trailing `6304XXXX` against the CRC of everything before the four hex digits.
pub fn check_crc(payload: &str) -> CrcCheck {
    let Some(body) = payload.len().checked_sub(4).and_then(|n| payload.get(..n)) else { return CrcCheck::Missing };
    if !body.ends_with("6304") {
        return CrcCheck::Missing;
    }
    let found = payload[body.len()..].to_uppercase();
    let expected = crc16(body);
    if found == expected { CrcCheck::Valid } else { CrcCheck::Invalid { expected, found } }
}

/// Recomputes the CRC, keeping everything before tag 63.
pub fn fix_crc(payload: &str) -> String {
    let body = match payload.rfind("6304") {
        Some(at) if at + 8 == payload.len() || at + 4 == payload.len() => &payload[..at],
        _ => payload,
    };
    let body = format!("{body}6304");
    let crc = crc16(&body);
    body + &crc
}

pub fn tag_name(tag: &str) -> &'static str {
    match tag.parse::<u8>() {
        Ok(2..=25) => return "Card network account",
        Ok(26..=50) => return "Merchant account",
        _ => {}
    }
    match tag {
        "00" => "Payload format",
        "01" => "Point of initiation",
        "51" => "Merchant account (domestic, NMID)",
        "52" => "Merchant category code",
        "53" => "Currency",
        "54" => "Amount",
        "55" => "Tip indicator",
        "56" => "Tip, fixed",
        "57" => "Tip, percent",
        "58" => "Country",
        "59" => "Merchant name",
        "60" => "City",
        "61" => "Postal code",
        "62" => "Additional data",
        "63" => "CRC",
        "64" => "Merchant info, alternate language",
        _ => "",
    }
}

/// Names of sub-fields inside template `parent`.
pub fn sub_tag_name(parent: &str, tag: &str) -> &'static str {
    match (parent, tag) {
        ("62", "01") => "Bill number",
        ("62", "02") => "Mobile number",
        ("62", "03") => "Store label",
        ("62", "04") => "Loyalty number",
        ("62", "05") => "Reference label",
        ("62", "06") => "Customer label",
        ("62", "07") => "Terminal label",
        ("62", "08") => "Purpose",
        ("62", "09") => "Consumer data request",
        ("64", "00") => "Language",
        ("64", "01") => "Merchant name",
        ("64", "02") => "City",
        ("51", "02") => "NMID",
        (_, "00") => "Global unique ID",
        (_, "01") => "Merchant PAN",
        (_, "02") => "Merchant ID",
        (_, "03") => "Merchant criteria",
        _ => "",
    }
}

/// Human description of a value where the raw one is a code.
pub fn describe(tag: &str, value: &str) -> Option<&'static str> {
    match (tag, value) {
        ("01", "11") => Some("static, customer enters the amount"),
        ("01", "12") => Some("dynamic, amount is in the code"),
        ("53", "360") => Some("IDR"),
        ("55", "01") => Some("customer is asked for a tip"),
        ("55", "02") => Some("fixed tip"),
        ("55", "03") => Some("percentage tip"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_the_standard_check_value() {
        assert_eq!(crc16("123456789"), "29B1");
    }

    #[test]
    fn parses_nested_templates_and_rejects_bad_lengths() {
        let payload = format!("{}{}{}", tlv("00", "01"), tlv("26", &(tlv("00", "ID.CO.QRIS.WWW") + &tlv("01", "123"))), tlv("59", "TOKO"));
        let fields = parse(&payload).unwrap();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[1].children[1], Field { tag: "01".into(), value: "123".into(), children: vec![] });
        assert!(fields[2].children.is_empty());
        assert!(parse("0005AB").is_err());
        assert!(parse("0X02AB").is_err());
    }

    #[test]
    fn crc_check_and_fix() {
        let body = format!("{}6304", tlv("00", "01"));
        let good = format!("{body}{}", crc16(&body));
        assert_eq!(check_crc(&good), CrcCheck::Valid);
        let bad = format!("{body}0000");
        assert!(matches!(check_crc(&bad), CrcCheck::Invalid { .. }));
        assert_eq!(fix_crc(&bad), good);
        assert_eq!(check_crc("000201"), CrcCheck::Missing);
    }
}
