//! Bad photos on purpose: degrade a KTP card (or any image) the ways real KYC uploads go wrong, to
//! test that OCR and liveness checks reject or recover them.

use std::f32::consts::PI;
use std::path::PathBuf;
use anyhow::{Context, Result, bail};
use image::{Rgba, RgbaImage};
use rand::Rng as _;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::generator::{Generator, Rng, rng, to_json};
use crate::render::Canvas;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Input {
    Generated { generator: String, params: Value, seed: u64 },
    File { path: PathBuf },
}

impl Default for Input {
    fn default() -> Self {
        Input::Generated { generator: "ktp".into(), params: Value::Object(Default::default()), seed: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    /// Out of focus.
    Blur { sigma: f32 },
    /// Motion blur along a direction.
    Shake { length: u32, angle: f32 },
    /// A lamp or window reflected on the laminate; centre as fractions of the image.
    Glare { x: f32, y: f32, radius: f32, strength: f32 },
    /// Held at an angle, on a dark table.
    Tilt { degrees: f32 },
    /// Under-exposed: 0 = unchanged, 1 = black.
    Dark { amount: f32 },
    /// Over-exposed / washed out.
    Bright { amount: f32 },
    /// Part of the card outside the frame; fractions cut from each side.
    Cut { left: f32, top: f32, right: f32, bottom: f32 },
    /// Heavy JPEG compression.
    Jpeg { quality: u8 },
    /// Photographing a screen: interference stripes.
    Moire { period: f32, strength: f32 },
    /// Sensor noise in low light.
    Noise { amount: f32 },
    /// A low-resolution camera, scaled back up.
    LowRes { factor: f32 },
}

impl Effect {
    pub const PRESETS: [(&'static str, Effect); 11] = [
        ("Blur", Effect::Blur { sigma: 4.0 }),
        ("Shake", Effect::Shake { length: 24, angle: 15.0 }),
        ("Glare", Effect::Glare { x: 0.35, y: 0.4, radius: 0.35, strength: 0.9 }),
        ("Tilt", Effect::Tilt { degrees: 18.0 }),
        ("Too dark", Effect::Dark { amount: 0.75 }),
        ("Washed out", Effect::Bright { amount: 0.6 }),
        ("Cut off", Effect::Cut { left: 0.0, top: 0.0, right: 0.3, bottom: 0.15 }),
        ("JPEG artifacts", Effect::Jpeg { quality: 6 }),
        ("Moiré", Effect::Moire { period: 6.0, strength: 0.35 }),
        ("Noise", Effect::Noise { amount: 0.25 }),
        ("Low resolution", Effect::LowRes { factor: 0.15 }),
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Effect::Blur { .. } => "Blur",
            Effect::Shake { .. } => "Shake",
            Effect::Glare { .. } => "Glare",
            Effect::Tilt { .. } => "Tilt",
            Effect::Dark { .. } => "Too dark",
            Effect::Bright { .. } => "Washed out",
            Effect::Cut { .. } => "Cut off",
            Effect::Jpeg { .. } => "JPEG artifacts",
            Effect::Moire { .. } => "Moiré",
            Effect::Noise { .. } => "Noise",
            Effect::LowRes { .. } => "Low resolution",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NegativeParams {
    pub input: Input,
    /// Applied in order.
    pub effects: Vec<Effect>,
}

impl Default for NegativeParams {
    fn default() -> Self {
        Self { input: Input::default(), effects: vec![Effect::PRESETS[2].1] }
    }
}

pub fn load_input(input: &Input) -> Result<RgbaImage> {
    match input {
        Input::File { path } => Ok(image::open(path).with_context(|| format!("couldn't open {}", path.display()))?.to_rgba8()),
        Input::Generated { generator, params, seed } => {
            if generator == KycNegative::ID {
                bail!("a KYC negative can't use another KYC negative as its input");
            }
            let g = crate::registry::find(generator).with_context(|| format!("unknown generator {generator:?}"))?;
            (g.generate)(params, *seed)
        }
    }
}

pub fn apply(image: RgbaImage, effect: &Effect, seed: u64) -> Result<RgbaImage> {
    let (w, h) = image.dimensions();
    Ok(match *effect {
        Effect::Blur { sigma } => image::imageops::blur(&image, sigma.clamp(0.1, 40.0)),
        Effect::Shake { length, angle } => shake(&image, length.clamp(1, 200), angle),
        Effect::Glare { x, y, radius, strength } => glare(image, (x * w as f32, y * h as f32), radius * w.max(h) as f32, strength),
        Effect::Tilt { degrees } => tilt(&image, degrees),
        Effect::Dark { amount } => map_channels(image, |v| v * (1.0 - amount.clamp(0.0, 1.0)).powf(1.4)),
        Effect::Bright { amount } => map_channels(image, |v| v + (255.0 - v) * amount.clamp(0.0, 1.0)),
        Effect::Cut { left, top, right, bottom } => {
            let region = crate::vcam::scene::Crop { left, top, right, bottom }.region(w as f32, h as f32);
            image::imageops::crop_imm(&image, region.0 as u32, region.1 as u32, region.2 as u32, region.3 as u32).to_image()
        }
        Effect::Jpeg { quality } => jpeg(&image, quality.clamp(1, 100))?,
        Effect::Moire { period, strength } => moire(image, period.max(1.5), strength),
        Effect::Noise { amount } => noise(image, amount, seed),
        Effect::LowRes { factor } => {
            let f = factor.clamp(0.02, 1.0);
            let small = image::imageops::resize(&image, ((w as f32 * f) as u32).max(1), ((h as f32 * f) as u32).max(1), image::imageops::FilterType::Triangle);
            image::imageops::resize(&small, w, h, image::imageops::FilterType::Triangle)
        }
    })
}

fn map_channels(mut image: RgbaImage, f: impl Fn(f32) -> f32) -> RgbaImage {
    for p in image.pixels_mut() {
        for c in &mut p.0[..3] {
            *c = f(f32::from(*c)).clamp(0.0, 255.0) as u8;
        }
    }
    image
}

fn glare(mut image: RgbaImage, center: (f32, f32), radius: f32, strength: f32) -> RgbaImage {
    let radius = radius.max(1.0);
    for (x, y, p) in image.enumerate_pixels_mut() {
        let d = ((x as f32 - center.0).powi(2) + (y as f32 - center.1).powi(2)).sqrt() / radius;
        let light = (1.0 - d).max(0.0).powi(2) * strength.clamp(0.0, 1.5);
        for c in &mut p.0[..3] {
            *c = (f32::from(*c) + (255.0 - f32::from(*c)) * light.min(1.0)).min(255.0) as u8;
        }
    }
    image
}

fn shake(image: &RgbaImage, length: u32, angle: f32) -> RgbaImage {
    let (dx, dy) = (angle.to_radians().cos(), angle.to_radians().sin());
    let (w, h) = image.dimensions();
    let taps = length.min(64);
    RgbaImage::from_fn(w, h, |x, y| {
        let mut sum = [0u32; 4];
        for i in 0..taps {
            let t = i as f32 * length as f32 / taps as f32 - length as f32 / 2.0;
            let sx = (x as f32 + dx * t).clamp(0.0, (w - 1) as f32) as u32;
            let sy = (y as f32 + dy * t).clamp(0.0, (h - 1) as f32) as u32;
            for (s, c) in sum.iter_mut().zip(image.get_pixel(sx, sy).0) {
                *s += u32::from(c);
            }
        }
        Rgba(sum.map(|s| (s / taps) as u8))
    })
}

fn tilt(image: &RgbaImage, degrees: f32) -> RgbaImage {
    let (w, h) = (image.width() as f32, image.height() as f32);
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (bw, bh) = (w * cos.abs() + h * sin.abs(), w * sin.abs() + h * cos.abs());
    let mut canvas = Canvas::new(bw.ceil() as u32, bh.ceil() as u32, (38, 34, 30));
    let source = Canvas::from_rgba(image);
    let transform = tiny_skia::Transform::from_translate(bw / 2.0, bh / 2.0).pre_rotate(degrees).pre_translate(-w / 2.0, -h / 2.0);
    canvas.draw(&source.pixmap, transform, None);
    canvas.to_rgba()
}

fn jpeg(image: &RgbaImage, quality: u8) -> Result<RgbaImage> {
    let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality).encode_image(&rgb)?;
    Ok(image::load_from_memory(&bytes)?.to_rgba8())
}

fn moire(mut image: RgbaImage, period: f32, strength: f32) -> RgbaImage {
    // Two gratings a few degrees apart beat against each other, like a camera against a screen's pixels.
    let (a, b) = (0.0f32.to_radians(), 4.0f32.to_radians());
    for (x, y, p) in image.enumerate_pixels_mut() {
        let (fx, fy) = (x as f32, y as f32);
        let g1 = (2.0 * PI * (fx * a.cos() + fy * a.sin()) / period).sin();
        let g2 = (2.0 * PI * (fx * b.cos() + fy * b.sin()) / (period * 1.03)).sin();
        let shade = 1.0 - strength.clamp(0.0, 1.0) * 0.5 * (1.0 + g1 * g2);
        for c in &mut p.0[..3] {
            *c = (f32::from(*c) * shade) as u8;
        }
    }
    image
}

fn noise(mut image: RgbaImage, amount: f32, seed: u64) -> RgbaImage {
    let mut r = rng(seed ^ 0x5DEE_CE66);
    let spread = amount.clamp(0.0, 1.0) * 120.0;
    for p in image.pixels_mut() {
        // Sum of uniforms: cheap and close enough to Gaussian for grain.
        let n: f32 = (0..3).map(|_| r.random_range(-1.0f32..1.0)).sum::<f32>() / 3.0 * spread;
        for c in &mut p.0[..3] {
            *c = (f32::from(*c) + n).clamp(0.0, 255.0) as u8;
        }
    }
    image
}

pub struct KycNegative;

impl Generator for KycNegative {
    const ID: &'static str = "kyc-negative";
    const NAME: &'static str = "KYC negative";
    type Params = NegativeParams;

    fn randomize(rng: &mut Rng) -> NegativeParams {
        let ktp = crate::ktp::KtpParams::random(rng, crate::ktp::today());
        let count = rng.random_range(1..=2);
        let effects = Effect::PRESETS.choose_multiple(rng, count).map(|(_, e)| *e).collect();
        NegativeParams { input: Input::Generated { generator: "ktp".into(), params: to_json(&ktp), seed: rng.random() }, effects }
    }

    fn generate(params: &NegativeParams, seed: u64) -> Result<RgbaImage> {
        let mut image = load_input(&params.input)?;
        for (i, effect) in params.effects.iter().enumerate() {
            image = apply(image, effect, seed.wrapping_add(i as u64))?;
        }
        Ok(image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> RgbaImage {
        let mut image = RgbaImage::from_pixel(200, 120, Rgba([120, 180, 220, 255]));
        for x in 0..200 {
            image.put_pixel(x, 60, Rgba([0, 0, 0, 255]));
        }
        image
    }

    fn mean(image: &RgbaImage) -> f32 {
        image.pixels().map(|p| f32::from(p.0[0]) + f32::from(p.0[1]) + f32::from(p.0[2])).sum::<f32>() / (image.pixels().len() as f32 * 3.0)
    }

    #[test]
    fn every_preset_applies() {
        for (name, effect) in Effect::PRESETS {
            let out = apply(card(), &effect, 1).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(out.width() > 0 && out.height() > 0, "{name}");
            assert_ne!(out.as_raw(), card().as_raw(), "{name} changed nothing");
        }
    }

    #[test]
    fn effects_move_brightness_the_right_way() {
        let base = mean(&card());
        assert!(mean(&apply(card(), &Effect::Dark { amount: 0.7 }, 0).unwrap()) < base * 0.5);
        assert!(mean(&apply(card(), &Effect::Bright { amount: 0.7 }, 0).unwrap()) > base);
        let tilted = apply(card(), &Effect::Tilt { degrees: 30.0 }, 0).unwrap();
        assert!(tilted.width() > 200 && tilted.height() > 120);
        let cut = apply(card(), &Effect::Cut { left: 0.5, top: 0.0, right: 0.0, bottom: 0.0 }, 0).unwrap();
        assert_eq!(cut.dimensions(), (100, 120));
    }

    #[test]
    fn generates_from_a_ktp_and_refuses_itself_as_input() {
        let mut params = KycNegative::randomize(&mut rng(4));
        params.effects = vec![Effect::Blur { sigma: 2.0 }];
        let image = KycNegative::generate(&params, 0).unwrap();
        assert_eq!(image.dimensions(), (crate::ktp::card::WIDTH, crate::ktp::card::HEIGHT));
        let looped = NegativeParams { input: Input::Generated { generator: "kyc-negative".into(), params: Value::Null, seed: 0 }, effects: vec![] };
        assert!(KycNegative::generate(&looped, 0).is_err());
    }
}
