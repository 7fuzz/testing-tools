//! Fake Indonesian people for form and API test data: NIK, NPWP, phone, email, address.

pub mod npwp;
pub mod phone;

use chrono::NaiveDate;
use rand::Rng as _;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use crate::generator::{Rng, rng};
use crate::ktp::identity::{self, Gender, IdentityParams};

/// Reserved for documentation and testing (RFC 2606 / 6761), so mail to them never reaches anyone.
pub const EMAIL_DOMAINS: &[&str] = &["example.com", "example.org", "example.net", "mail.test"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    pub nik: String,
    pub npwp: String,
    pub gender: Gender,
    pub birth_place: String,
    pub birth_date: NaiveDate,
    pub phone: String,
    pub operator: String,
    pub email: String,
    pub address: String,
    pub rtrw: String,
    pub village: String,
    pub district: String,
    pub city: String,
    pub province: String,
    pub religion: String,
    pub marital: String,
    pub job: String,
}

impl Person {
    pub const COLUMNS: [&'static str; 18] = [
        "name", "nik", "npwp", "gender", "birth_place", "birth_date", "phone", "operator", "email", "address", "rtrw", "village",
        "district", "city", "province", "religion", "marital", "job",
    ];

    pub fn field(&self, column: &str) -> String {
        match column {
            "name" => self.name.clone(),
            "nik" => self.nik.clone(),
            "npwp" => self.npwp.clone(),
            "gender" => self.gender.label().into(),
            "birth_place" => self.birth_place.clone(),
            "birth_date" => self.birth_date.format("%Y-%m-%d").to_string(),
            "phone" => self.phone.clone(),
            "operator" => self.operator.clone(),
            "email" => self.email.clone(),
            "address" => self.address.clone(),
            "rtrw" => self.rtrw.clone(),
            "village" => self.village.clone(),
            "district" => self.district.clone(),
            "city" => self.city.clone(),
            "province" => self.province.clone(),
            "religion" => self.religion.clone(),
            "marital" => self.marital.clone(),
            "job" => self.job.clone(),
            _ => String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PeopleParams {
    pub gender: Option<Gender>,
    /// A city from the KTP region list; `None` picks at random per person.
    pub city: Option<String>,
    pub phone_format: phone::Format,
    pub operator: Option<String>,
}

fn email_for(rng: &mut Rng, name: &str) -> String {
    let parts: Vec<String> = name.split_whitespace().map(|p| p.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "")).collect();
    let local = match rng.random_range(0..4) {
        0 => parts.join("."),
        1 => format!("{}{}", parts.first().cloned().unwrap_or_default(), rng.random_range(1..=999)),
        2 => format!("{}_{}", parts.first().cloned().unwrap_or_default(), parts.last().cloned().unwrap_or_default()),
        _ => format!("qa.{}{}", parts.concat(), rng.random_range(10..=99)),
    };
    format!("{local}@{}", EMAIL_DOMAINS.choose(rng).expect("domains"))
}

pub fn person(params: &PeopleParams, rng: &mut Rng, today: NaiveDate) -> Person {
    let mut given = IdentityParams { gender: params.gender, ..Default::default() };
    if let Some(region) = params.city.as_deref().and_then(identity::region_for_city) {
        let index = rng.random_range(0..region.districts.len());
        let (district, villages) = region.districts[index];
        given.province = Some(region.province.into());
        given.city = Some(region.city.into());
        given.region = Some(format!("{}{:02}", region.code, index + 1));
        given.district = Some(district.into());
        given.village = villages.choose(rng).map(|v| (*v).into());
    }
    let id = identity::resolve(&given, rng, today);
    let (number, operator) = phone::random(rng, params.operator.as_deref(), params.phone_format);
    Person {
        email: email_for(rng, &id.name),
        npwp: npwp::from_nik(&id.nik),
        name: id.name,
        nik: id.nik,
        gender: id.gender,
        birth_place: id.birth_place,
        birth_date: id.birth,
        phone: number,
        operator: operator.into(),
        address: id.address,
        rtrw: id.rtrw,
        village: id.village,
        district: id.district,
        city: id.city,
        province: id.province,
        religion: id.religion,
        marital: id.marital,
        job: id.job,
    }
}

/// `count` people from `seed`; the same arguments always give the same list.
pub fn people(params: &PeopleParams, count: usize, seed: u64, today: NaiveDate) -> Vec<Person> {
    let mut rng = rng(seed);
    (0..count).map(|_| person(params, &mut rng, today)).collect()
}

fn csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n']) { format!("\"{}\"", value.replace('"', "\"\"")) } else { value.to_string() }
}

pub fn to_csv(people: &[Person], columns: &[&str]) -> String {
    let mut out = columns.join(",");
    out.push('\n');
    for person in people {
        let row: Vec<String> = columns.iter().map(|c| csv_cell(&person.field(c))).collect();
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

pub fn to_json(people: &[Person], columns: &[&str]) -> String {
    let rows: Vec<serde_json::Map<String, serde_json::Value>> = people
        .iter()
        .map(|p| columns.iter().map(|c| ((*c).to_string(), serde_json::Value::String(p.field(c)))).collect())
        .collect();
    serde_json::to_string_pretty(&rows).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 13).unwrap()
    }

    #[test]
    fn people_are_reproducible_and_consistent() {
        let a = people(&PeopleParams::default(), 50, 9, today());
        assert_eq!(a, people(&PeopleParams::default(), 50, 9, today()));
        for p in &a {
            let nik = identity::parse_nik(&p.nik).unwrap();
            assert_eq!(nik.gender, p.gender);
            assert!(npwp::validate(&p.npwp).is_ok(), "{}", p.npwp);
            assert!(phone::operator_of(&p.phone).is_some(), "{}", p.phone);
            assert!(EMAIL_DOMAINS.iter().any(|d| p.email.ends_with(d)), "{}", p.email);
        }
    }

    #[test]
    fn filters_apply() {
        let params = PeopleParams { gender: Some(Gender::Female), city: Some("KOTA BANDUNG".into()), ..Default::default() };
        for p in people(&params, 20, 1, today()) {
            assert_eq!(p.gender, Gender::Female);
            assert_eq!(p.city, "KOTA BANDUNG");
            assert!(p.nik.starts_with("3273"));
        }
    }

    #[test]
    fn csv_quotes_and_json_keeps_columns() {
        let list = people(&PeopleParams::default(), 3, 2, today());
        let csv = to_csv(&list, &["name", "address"]);
        assert_eq!(csv.lines().count(), 4);
        assert!(csv.starts_with("name,address\n"));
        let json: Vec<serde_json::Value> = serde_json::from_str(&to_json(&list, &["nik"])).unwrap();
        assert_eq!(json.len(), 3);
        assert_eq!(json[0].as_object().unwrap().len(), 1);
        assert_eq!(csv_cell("a,\"b\""), "\"a,\"\"b\"\"\"");
    }
}
