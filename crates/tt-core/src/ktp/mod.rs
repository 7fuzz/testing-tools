//! KTP-layout test cards for staging KYC, ported from dg.ewallet/tools/ktp_cat.py.
//!
//! The output is deliberately not a forgery: no Garuda emblem or official seal, a cat instead of a
//! person, and a SPECIMEN mark that can't be turned off.

pub mod card;
pub mod cat;
pub mod data;
pub mod identity;

use std::path::PathBuf;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use crate::generator::{Generator, Rng, rng};
use crate::render::Canvas;
use cat::CatLook;
use identity::{Gender, Identity, IdentityParams};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KtpParams {
    #[serde(flatten)]
    pub identity: IdentityParams,
    /// `None` picks a random cat from the seed.
    pub look: Option<CatLook>,
    /// A photo to use instead of the drawn cat (it should be a cat).
    pub photo: Option<PathBuf>,
}

pub struct Resolved {
    pub identity: Identity,
    pub look: CatLook,
    pub photo: Option<RgbaImage>,
}

// Separate streams, so editing one field doesn't reshuffle the cat or the card background.
const LOOK_STREAM: u64 = 0x9E37_79B9_7F4A_7C15;
const ART_STREAM: u64 = 0xBF58_476D_1CE4_E5B9;
const SELFIE_STREAM: u64 = 0x94D0_49BB_1331_11EB;

pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

impl KtpParams {
    pub fn resolve(&self, seed: u64, today: NaiveDate) -> Result<Resolved> {
        let identity = identity::resolve(&self.identity, &mut rng(seed), today);
        let look = self.look.clone().unwrap_or_else(|| CatLook::random(&mut rng(seed ^ LOOK_STREAM)));
        let photo = match &self.photo {
            Some(path) => Some(image::open(path).with_context(|| format!("couldn't open photo {}", path.display()))?.to_rgba8()),
            None => None,
        };
        Ok(Resolved { identity, look, photo })
    }

    /// A random identity and cat with every field set, so the card no longer depends on today's date.
    pub fn random(rng: &mut Rng, today: NaiveDate) -> Self {
        let identity = identity::resolve(&IdentityParams::default(), rng, today);
        Self { identity: IdentityParams::pinned(&identity), look: Some(CatLook::random(rng)), photo: None }
    }
}

pub fn render_card(params: &KtpParams, seed: u64) -> Result<(Resolved, Canvas)> {
    let resolved = params.resolve(seed, today())?;
    let card = card::render_card(&resolved.identity, &resolved.look, resolved.photo.as_ref(), &mut rng(seed ^ ART_STREAM));
    Ok((resolved, card))
}

pub struct Rendered {
    pub identity: Identity,
    pub card: RgbaImage,
    pub selfie: Option<RgbaImage>,
}

/// The card, and the selfie holding that same card when `with_selfie` is set.
pub fn render_set(params: &KtpParams, seed: u64, with_selfie: bool) -> Result<Rendered> {
    let (resolved, card) = render_card(params, seed)?;
    let selfie = with_selfie
        .then(|| card::render_selfie(&card, &resolved.look, resolved.photo.as_ref(), &mut rng(seed ^ SELFIE_STREAM)).to_rgba());
    Ok(Rendered { identity: resolved.identity, card: card.to_rgba(), selfie })
}

pub struct Ktp;

impl Generator for Ktp {
    const ID: &'static str = "ktp";
    const NAME: &'static str = "KTP specimen";
    type Params = KtpParams;

    fn randomize(rng: &mut Rng) -> KtpParams {
        KtpParams::random(rng, today())
    }

    fn generate(params: &KtpParams, seed: u64) -> Result<RgbaImage> {
        Ok(render_card(params, seed)?.1.to_rgba())
    }
}

pub struct KtpSelfie;

impl Generator for KtpSelfie {
    const ID: &'static str = "ktp-selfie";
    const NAME: &'static str = "KTP selfie";
    type Params = KtpParams;

    fn randomize(rng: &mut Rng) -> KtpParams {
        KtpParams::random(rng, today())
    }

    fn generate(params: &KtpParams, seed: u64) -> Result<RgbaImage> {
        Ok(render_set(params, seed, true)?.selfie.expect("selfie requested"))
    }
}

/// The identity in EKTP-Generator's `data.json` field names.
pub fn to_ektp_json(id: &Identity) -> Value {
    json!({
        "provinsi": id.province,
        "kota": id.city,
        "nik": id.nik,
        "nama": id.name,
        "ttl": format!("{}, {}", id.birth_place, id.birth.format("%d-%m-%Y")),
        "jenis_kelamin": id.gender.label(),
        "golongan_darah": id.blood,
        "alamat": id.address,
        "rt/rw": id.rtrw,
        "kel/desa": id.village,
        "kecamatan": id.district,
        "agama": id.religion,
        "status": id.marital,
        "pekerjaan": id.job,
        "kewarganegaraan": "WNI",
        "masa_berlaku": "SEUMUR HIDUP",
        "terbuat": id.issued.format("%d-%m-%Y").to_string(),
    })
}

/// Reads EKTP-Generator's `data.json` fields; anything missing or unreadable stays random.
pub fn from_ektp_json(value: &Value) -> IdentityParams {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(|s| s.trim().to_uppercase()).filter(|s| !s.is_empty());
    let date = |s: &str| NaiveDate::parse_from_str(s.trim(), "%d-%m-%Y").ok();
    let (birth_place, birth) = match text("ttl") {
        Some(ttl) => match ttl.rsplit_once(',') {
            Some((place, when)) => (Some(place.trim().to_string()), date(when)),
            None => (Some(ttl), None),
        },
        None => (None, None),
    };
    let nik = text("nik").filter(|n| identity::parse_nik(n).is_ok());
    IdentityParams {
        province: text("provinsi"),
        city: text("kota"),
        region: nik.as_ref().map(|n| n[..6].to_string()),
        district: text("kecamatan"),
        village: text("kel/desa"),
        name: text("nama"),
        nik,
        birth_place,
        birth,
        gender: text("jenis_kelamin").and_then(|g| Gender::parse(&g)),
        blood: text("golongan_darah"),
        address: text("alamat"),
        rtrw: text("rt/rw"),
        religion: text("agama"),
        marital: text("status"),
        job: text("pekerjaan"),
        issued: text("terbuat").and_then(|d| date(&d)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed() -> KtpParams {
        let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
        KtpParams::random(&mut rng(5), today)
    }

    #[test]
    fn same_params_and_seed_give_the_same_png() {
        let params = fixed();
        let a = Ktp::generate(&params, 11).unwrap();
        let b = Ktp::generate(&params, 11).unwrap();
        assert_eq!(a.dimensions(), (card::WIDTH, card::HEIGHT));
        assert_eq!(a.as_raw(), b.as_raw());
        assert_ne!(a.as_raw(), Ktp::generate(&params, 12).unwrap().as_raw());
    }

    #[test]
    fn selfie_has_the_portrait_size() {
        let image = KtpSelfie::generate(&fixed(), 3).unwrap();
        assert_eq!(image.dimensions(), (card::SELFIE_WIDTH, card::SELFIE_HEIGHT));
    }

    #[test]
    fn every_cat_pattern_draws() {
        for seed in 0..40 {
            let look = CatLook::random(&mut rng(seed));
            let cat = cat::draw(210, 270, &look).to_rgba();
            assert_eq!(cat.dimensions(), (210, 270));
        }
    }

    #[test]
    fn ektp_json_round_trips() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
        let id = identity::resolve(&IdentityParams::default(), &mut rng(8), today);
        let params = from_ektp_json(&to_ektp_json(&id));
        assert_eq!(identity::resolve(&params, &mut rng(1), today), id);
    }

    #[test]
    fn reads_ektp_generator_sample() {
        let sample = json!({
            "provinsi": "jakarta selatan", "kota": "jakarta", "nik": "127xxxx", "nama": "kiana dimitri",
            "ttl": "jakarta, 01-09-2000", "jenis_kelamin": "laki-laki", "golongan_darah": "o",
            "rt/rw": "000/000", "terbuat": "19-10-2023"
        });
        let params = from_ektp_json(&sample);
        assert_eq!(params.name.as_deref(), Some("KIANA DIMITRI"));
        assert_eq!(params.birth, NaiveDate::from_ymd_opt(2000, 9, 1));
        assert_eq!(params.gender, Some(Gender::Male));
        assert_eq!(params.nik, None);
    }

    #[test]
    #[ignore = "writes sample images to target/screenshots for eyeballing"]
    fn write_samples() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
        std::fs::create_dir_all(&dir).unwrap();
        for seed in 0..3 {
            let params = KtpParams::random(&mut rng(seed + 100), today());
            Ktp::generate(&params, seed).unwrap().save(dir.join(format!("ktp-card-{seed}.png"))).unwrap();
            KtpSelfie::generate(&params, seed).unwrap().save(dir.join(format!("ktp-selfie-{seed}.png"))).unwrap();
        }
    }
}
