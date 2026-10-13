use chrono::{Datelike, Duration, NaiveDate};
use rand::Rng as _;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use super::data::{self, Region};
use crate::generator::Rng;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gender {
    #[default]
    #[serde(rename = "LAKI-LAKI")]
    Male,
    #[serde(rename = "PEREMPUAN")]
    Female,
}

impl Gender {
    pub const ALL: [Gender; 2] = [Gender::Male, Gender::Female];

    pub fn label(self) -> &'static str {
        match self {
            Gender::Male => "LAKI-LAKI",
            Gender::Female => "PEREMPUAN",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_uppercase().as_str() {
            "LAKI-LAKI" | "L" | "MALE" => Some(Gender::Male),
            "PEREMPUAN" | "P" | "FEMALE" => Some(Gender::Female),
            _ => None,
        }
    }
}

/// Every field printed on the card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    pub province: String,
    pub city: String,
    /// 6-digit wilayah code (province, city, district) the NIK starts with.
    pub region: String,
    pub district: String,
    pub village: String,
    pub name: String,
    pub nik: String,
    pub birth_place: String,
    pub birth: NaiveDate,
    pub gender: Gender,
    pub blood: String,
    pub address: String,
    pub rtrw: String,
    pub religion: String,
    pub marital: String,
    pub job: String,
    pub issued: NaiveDate,
}

/// Card fields; any left `None` are filled from the seed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IdentityParams {
    pub province: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub district: Option<String>,
    pub village: Option<String>,
    pub name: Option<String>,
    /// `None` derives it from region, birth date and gender.
    pub nik: Option<String>,
    pub birth_place: Option<String>,
    pub birth: Option<NaiveDate>,
    pub gender: Option<Gender>,
    pub blood: Option<String>,
    pub address: Option<String>,
    pub rtrw: Option<String>,
    pub religion: Option<String>,
    pub marital: Option<String>,
    pub job: Option<String>,
    pub issued: Option<NaiveDate>,
}

impl IdentityParams {
    /// Every field set (NIK still derived), so the identity no longer depends on the seed or today's date.
    pub fn pinned(identity: &Identity) -> Self {
        Self {
            province: Some(identity.province.clone()),
            city: Some(identity.city.clone()),
            region: Some(identity.region.clone()),
            district: Some(identity.district.clone()),
            village: Some(identity.village.clone()),
            name: Some(identity.name.clone()),
            nik: None,
            birth_place: Some(identity.birth_place.clone()),
            birth: Some(identity.birth),
            gender: Some(identity.gender),
            blood: Some(identity.blood.clone()),
            address: Some(identity.address.clone()),
            rtrw: Some(identity.rtrw.clone()),
            religion: Some(identity.religion.clone()),
            marital: Some(identity.marital.clone()),
            job: Some(identity.job.clone()),
            issued: Some(identity.issued),
        }
    }
}

/// NIK: region (6), birth day (+40 for women), month, 2-digit year, then a 4-digit serial.
pub fn make_nik(region: &str, birth: NaiveDate, gender: Gender, serial: u32) -> String {
    let day = birth.day() + if gender == Gender::Female { 40 } else { 0 };
    format!("{region}{day:02}{:02}{:02}{:04}", birth.month(), birth.year().rem_euclid(100), serial % 10_000)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NikInfo {
    pub region: String,
    pub birth_day: u32,
    pub birth_month: u32,
    pub birth_year_2: u32,
    pub gender: Gender,
    pub serial: String,
}

/// Splits a NIK into its parts; `Err` says what is wrong with it.
pub fn parse_nik(nik: &str) -> Result<NikInfo, String> {
    if nik.len() != 16 || !nik.bytes().all(|b| b.is_ascii_digit()) {
        return Err("a NIK is 16 digits".into());
    }
    let num = |range: std::ops::Range<usize>| nik[range].parse::<u32>().expect("digits");
    let (raw_day, month, year) = (num(6..8), num(8..10), num(10..12));
    let (gender, day) = if raw_day > 40 { (Gender::Female, raw_day - 40) } else { (Gender::Male, raw_day) };
    if !(1..=31).contains(&day) {
        return Err(format!("birth day {raw_day} is out of range (01–31, or 41–71 for women)"));
    }
    if !(1..=12).contains(&month) {
        return Err(format!("birth month {month} is out of range"));
    }
    if num(12..16) == 0 {
        return Err("serial 0000 is never issued".into());
    }
    Ok(NikInfo { region: nik[..6].into(), birth_day: day, birth_month: month, birth_year_2: year, gender, serial: nik[12..].into() })
}

fn weighted<'a>(rng: &mut Rng, options: &[(&'a str, u32)]) -> &'a str {
    let total: u32 = options.iter().map(|(_, w)| w).sum();
    let mut roll = rng.random_range(0..total.max(1));
    for &(option, weight) in options {
        if roll < weight {
            return option;
        }
        roll -= weight;
    }
    options[0].0
}

fn pick<'a>(rng: &mut Rng, options: &[&'a str]) -> &'a str {
    options.choose(rng).copied().unwrap_or_default()
}

fn random_name(rng: &mut Rng, gender: Gender) -> String {
    let (firsts, lasts) = match gender {
        Gender::Male => (data::FIRST_NAMES_MALE, data::LAST_NAMES_MALE),
        Gender::Female => (data::FIRST_NAMES_FEMALE, data::LAST_NAMES_FEMALE),
    };
    let first = pick(rng, firsts);
    if rng.random_bool(0.1) {
        return first.into();
    }
    let mut parts = vec![first];
    if rng.random_bool(0.35) {
        let others: Vec<&str> = firsts.iter().copied().filter(|n| *n != first).collect();
        parts.push(pick(rng, &others));
    }
    let all_lasts: Vec<&str> = lasts.iter().chain(data::LAST_NAMES_ANY).copied().collect();
    parts.push(pick(rng, &all_lasts));
    parts.join(" ")
}

fn random_address(rng: &mut Rng) -> String {
    let n = rng.random_range(1..=150);
    let street = pick(rng, data::STREETS);
    let housing = pick(rng, data::HOUSING);
    match rng.random_range(0..6) {
        0 => format!("JL. {street} NO. {n}"),
        1 => format!("JL. {street} NO. {n}{}", pick(rng, &["A", "B", "C"])),
        2 => format!("GG. {street} NO. {n}"),
        3 => format!(
            "KOMP. {housing} BLOK {}{} NO. {}",
            pick(rng, &["A", "B", "C", "D", "E", "F", "G"]),
            rng.random_range(1..=12),
            rng.random_range(1..=30)
        ),
        4 => format!("PERUM {housing} NO. {n}"),
        _ => format!("KP. {street}"),
    }
}

fn random_marital(rng: &mut Rng, age: i64) -> String {
    if age < 20 {
        return weighted(rng, &[("BELUM KAWIN", 95), ("KAWIN", 5)]).into();
    }
    let single = (60 - age).max(5) as u32;
    let widowed = (age / 10).max(0) as u32;
    weighted(rng, &[("BELUM KAWIN", single), ("KAWIN", 60), ("CERAI HIDUP", 5), ("CERAI MATI", widowed)]).into()
}

fn random_job(rng: &mut Rng, age: i64, gender: Gender) -> String {
    if age < 23 && rng.random_bool(0.7) {
        return data::STUDENT.into();
    }
    if age > 58 && rng.random_bool(0.4) {
        return data::RETIRED.into();
    }
    if gender == Gender::Female && rng.random_bool(0.25) {
        return data::HOMEMAKER.into();
    }
    pick(rng, data::JOBS).into()
}

fn random_religion(rng: &mut Rng, province: &str) -> String {
    let options: &[(&str, u32)] = match province {
        "BALI" => &[("HINDU", 80), ("ISLAM", 12), ("KRISTEN", 4), ("KATHOLIK", 2), ("BUDHA", 2)],
        "SUMATERA UTARA" => &[("ISLAM", 55), ("KRISTEN", 30), ("KATHOLIK", 8), ("BUDHA", 6), ("KONGHUCU", 1)],
        _ => &[("ISLAM", 86), ("KRISTEN", 6), ("KATHOLIK", 3), ("HINDU", 2), ("BUDHA", 2), ("KONGHUCU", 1)],
    };
    weighted(rng, options).into()
}

fn random_date(rng: &mut Rng, start: NaiveDate, end: NaiveDate) -> NaiveDate {
    let days = (end - start).num_days().max(0);
    start + Duration::days(rng.random_range(0..=days))
}

/// The town name without its KOTA / KABUPATEN prefix, as printed under the photo.
pub fn town(city: &str) -> &str {
    city.trim_start_matches("KOTA ").trim_start_matches("KABUPATEN ")
}

pub fn region_for_city(city: &str) -> Option<&'static Region> {
    data::REGIONS.iter().find(|r| r.city == city)
}

/// Fills every unset field of `params`, following ktp_cat.py's rules: ages 17–70, an age-aware job and
/// marital status, religion weighted by province, and an issue date after the 17th birthday.
pub fn resolve(params: &IdentityParams, rng: &mut Rng, today: NaiveDate) -> Identity {
    let region = data::REGIONS.choose(rng).expect("regions");
    let district_index = rng.random_range(0..region.districts.len());
    let (district, villages) = region.districts[district_index];
    let village = pick(rng, villages);
    let gender = params.gender.unwrap_or_else(|| if rng.random_bool(0.5) { Gender::Male } else { Gender::Female });
    let birth = params.birth.unwrap_or_else(|| random_date(rng, today - Duration::days(70 * 365), today - Duration::days(17 * 366)));
    let age = (today - birth).num_days() / 365;
    let province = params.province.clone().unwrap_or_else(|| region.province.into());
    let home = town(region.city).to_string();
    let birth_place = if rng.random_bool(0.5) { home } else { pick(rng, data::BIRTH_PLACES).into() };
    let blood = weighted(rng, &[("-", 35), ("O", 25), ("A", 18), ("B", 17), ("AB", 5)]).to_string();
    let name = random_name(rng, gender);
    let address = random_address(rng);
    let rtrw = format!("{:03}/{:03}", rng.random_range(1..=15), rng.random_range(1..=12));
    let religion = random_religion(rng, &province);
    let marital = random_marital(rng, age);
    let job = random_job(rng, age, gender);
    let adult = birth + Duration::days(17 * 366);
    let issued = random_date(rng, adult.max(today - Duration::days(3650)), today);
    let serial = rng.random_range(1..=9999);
    let region_code = params.region.clone().unwrap_or_else(|| format!("{}{:02}", region.code, district_index + 1));
    let nik = params.nik.clone().unwrap_or_else(|| make_nik(&region_code, birth, gender, serial));

    Identity {
        province,
        city: params.city.clone().unwrap_or_else(|| region.city.into()),
        region: region_code,
        district: params.district.clone().unwrap_or_else(|| district.into()),
        village: params.village.clone().unwrap_or_else(|| village.into()),
        name: params.name.clone().unwrap_or(name),
        nik,
        birth_place: params.birth_place.clone().unwrap_or(birth_place),
        birth,
        gender,
        blood: params.blood.clone().unwrap_or(blood),
        address: params.address.clone().unwrap_or(address),
        rtrw: params.rtrw.clone().unwrap_or(rtrw),
        religion: params.religion.clone().unwrap_or(religion),
        marital: params.marital.clone().unwrap_or(marital),
        job: params.job.clone().unwrap_or(job),
        issued: params.issued.unwrap_or(issued),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::rng;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn nik_encodes_birth_and_gender() {
        assert_eq!(make_nik("357802", date(2003, 4, 29), Gender::Male, 1480), "3578022904031480");
        assert_eq!(make_nik("327301", date(1990, 6, 15), Gender::Female, 1), "3273015506900001");
        let info = parse_nik("3273015506900001").unwrap();
        assert_eq!((info.gender, info.birth_day, info.birth_month, info.birth_year_2), (Gender::Female, 15, 6, 90));
        assert!(parse_nik("3273019906900001").is_err());
        assert!(parse_nik("327301550690000").is_err());
        assert!(parse_nik("3273015506900000").is_err());
    }

    #[test]
    fn resolved_identities_follow_the_rules() {
        let today = date(2026, 10, 13);
        for seed in 0..200 {
            let id = resolve(&IdentityParams::default(), &mut rng(seed), today);
            let age = (today - id.birth).num_days() / 365;
            assert!((16..=70).contains(&age), "age {age}");
            assert!(id.issued >= id.birth + Duration::days(17 * 366) && id.issued <= today);
            let info = parse_nik(&id.nik).unwrap();
            assert_eq!(info.gender, id.gender);
            assert_eq!(info.region, id.region);
            assert!(region_for_city(&id.city).is_some());
        }
    }

    #[test]
    fn given_fields_win_and_pinning_freezes_the_identity() {
        let today = date(2026, 10, 13);
        let params = IdentityParams { name: Some("QA EWALLET B".into()), gender: Some(Gender::Female), ..Default::default() };
        let id = resolve(&params, &mut rng(1), today);
        assert_eq!(id.name, "QA EWALLET B");
        assert_eq!(id.gender, Gender::Female);
        let pinned = IdentityParams::pinned(&id);
        let later = resolve(&pinned, &mut rng(99), date(2030, 1, 1));
        assert_eq!(Identity { nik: id.nik.clone(), ..later.clone() }, id);
        assert_eq!(&later.nik[..12], &id.nik[..12]);
    }
}
