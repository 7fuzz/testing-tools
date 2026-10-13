use std::f32::consts::PI;
use image::RgbaImage;
use rand::Rng as _;
use rand::seq::IndexedRandom;
use tiny_skia::Transform;
use super::cat::{self, CatLook};
use super::identity::{self, Identity};
use crate::generator::Rng;
use crate::render::text::{self, Anchor, BOLD, Pen, REGULAR};
use crate::render::{self, Canvas, Rgb};

pub const WIDTH: u32 = 1011;
pub const HEIGHT: u32 = 638;
pub const SELFIE_WIDTH: u32 = 900;
pub const SELFIE_HEIGHT: u32 = 1200;
const NAVY: Rgb = (20, 20, 30);
const WALLS: [Rgb; 6] = [(225, 220, 210), (200, 210, 220), (235, 225, 200), (190, 200, 185), (215, 200, 205), (160, 150, 140)];
/// Always printed; this tool makes test specimens, never lookalike documents.
pub const SPECIMEN_MARK: &str = "SPECIMEN · DATA UJI";

fn background(rng: &mut Rng) -> Canvas {
    let mut bg = Canvas::new(WIDTH, HEIGHT, (0, 0, 0));
    let (tr, tg, tb) = (rng.random_range(-15..=15), rng.random_range(-15..=15), rng.random_range(-15..=15));
    for y in 0..HEIGHT {
        let t = y as f32 / HEIGHT as f32;
        let color = (
            (120.0 + tr as f32 + 60.0 * t) as u8,
            (185.0 + tg as f32 + 40.0 * t) as u8,
            (225.0 + tb as f32 + 20.0 * t) as u8,
        );
        for x in 0..WIDTH {
            bg.set_pixel(x, y, color);
        }
    }
    let stripe = ((150 + tr) as u8, (205 + tg) as u8, (235 + tb) as u8);
    let step = rng.random_range(14..=30) as usize;
    let h = HEIGHT as f32;
    match rng.random_range(0..4) {
        style @ 0..=2 => {
            for i in (0..(WIDTH + HEIGHT) as usize).step_by(step) {
                let i = i as f32;
                bg.line(&[(i, 0.0), (i - h, h)], 1.0, stripe, false);
                if style != 0 {
                    bg.line(&[(i - h, 0.0), (i, h)], 1.0, stripe, false);
                }
            }
        }
        _ => {
            let (amp, period) = (rng.random_range(6..=18) as f32, rng.random_range(80..=200) as f32);
            let mut y0 = -amp;
            while y0 < h + amp {
                let points: Vec<(f32, f32)> =
                    (0..=(WIDTH + 10) / 10).map(|i| (i as f32 * 10.0, y0 + amp * (2.0 * PI * i as f32 * 10.0 / period).sin())).collect();
                bg.line(&points, 1.0, stripe, false);
                y0 += step as f32;
            }
        }
    }
    bg.blur(rng.random_range(0.4..0.9));
    bg
}

fn signature(c: &mut Canvas, rng: &mut Rng, x: f32, y: f32) {
    let points: Vec<(f32, f32)> =
        (0..rng.random_range(4..=7)).map(|i| (x + i as f32 * rng.random_range(14..=30) as f32, y + rng.random_range(0..=38) as f32)).collect();
    c.line(&points, rng.random_range(2..=4) as f32, NAVY, true);
    if rng.random_bool(0.5) {
        let (first, last) = (points[0], points[points.len() - 1]);
        c.line(&[(first.0 - 5.0, last.1 + 6.0), (last.0 + 10.0, first.1 + 10.0)], 2.0, NAVY, true);
    }
}

/// The photo slot: the given photo cropped to fill, or a drawn cat.
pub fn photo(width: u32, height: u32, look: &CatLook, custom: Option<&RgbaImage>) -> Canvas {
    match custom {
        Some(image) => Canvas::from_rgba(&render::cover(image, width, height)),
        None => cat::draw(width, height, look),
    }
}

pub fn render_card(id: &Identity, look: &CatLook, custom_photo: Option<&RgbaImage>, rng: &mut Rng) -> Canvas {
    let mut c = background(rng);
    let center = WIDTH as f32 / 2.0;
    text::draw(&mut c, Pen::new(&BOLD, 30.0, NAVY), (center, 26.0), &format!("PROVINSI {}", id.province), Anchor::MiddleTop);
    text::draw(&mut c, Pen::new(&BOLD, 30.0, NAVY), (center, 64.0), &id.city, Anchor::MiddleTop);
    text::draw(&mut c, Pen::new(&BOLD, 34.0, NAVY), (40.0, 118.0), "NIK", Anchor::LeftAscender);
    text::draw(&mut c, Pen::new(&BOLD, 34.0, NAVY), (150.0, 118.0), ":", Anchor::LeftAscender);
    text::draw(&mut c, Pen::new(&BOLD, 40.0, NAVY), (175.0, 114.0), &id.nik, Anchor::LeftAscender);

    let rows = [
        ("Nama", id.name.clone()),
        ("Tempat/Tgl Lahir", format!("{}, {}", id.birth_place, id.birth.format("%d-%m-%Y"))),
        ("Jenis kelamin", format!("{}        Gol. Darah : {}", id.gender.label(), id.blood)),
        ("Alamat", id.address.clone()),
        ("     RT/RW", id.rtrw.clone()),
        ("     Kel/Desa", id.village.clone()),
        ("     Kecamatan", id.district.clone()),
        ("Agama", id.religion.clone()),
        ("Status Perkawinan", id.marital.clone()),
        ("Pekerjaan", id.job.clone()),
        ("Kewarganegaraan", "WNI".into()),
        ("Berlaku Hingga", "SEUMUR HIDUP".into()),
    ];
    let (px, py, pw, ph) = (760.0, 150.0, 210u32, 270u32);
    let mut y = 180.0;
    for (label, value) in rows {
        text::draw(&mut c, Pen::new(&REGULAR, 21.0, NAVY), (40.0, y), label, Anchor::LeftAscender);
        text::draw(&mut c, Pen::new(&REGULAR, 21.0, NAVY), (265.0, y), ":", Anchor::LeftAscender);
        let value = value.to_uppercase();
        let size = text::fit_size(&REGULAR, 21.0, 12.0, &value, px - 285.0 - 15.0);
        text::draw(&mut c, Pen::new(&REGULAR, size, NAVY), (285.0, y), &value, Anchor::LeftAscender);
        y += 34.0;
    }

    let portrait = photo(pw, ph, look, custom_photo);
    c.draw(&portrait.pixmap, Transform::from_translate(px, py), None);
    c.stroke_rect(px, py, px + pw as f32 + 1.0, py + ph as f32 + 1.0, 2.0, NAVY);
    let mid = px + pw as f32 / 2.0;
    let below = py + ph as f32;
    text::draw(&mut c, Pen::new(&REGULAR, 19.0, NAVY), (mid, below + 14.0), identity::town(&id.city), Anchor::MiddleTop);
    text::draw(&mut c, Pen::new(&REGULAR, 19.0, NAVY), (mid, below + 40.0), &id.issued.format("%d-%m-%Y").to_string(), Anchor::MiddleTop);
    let sx = px + rng.random_range(25..=60) as f32;
    signature(&mut c, rng, sx, below + 70.0);

    text::draw(&mut c, Pen { font: &BOLD, size: 18.0, color: (200, 30, 30), alpha: 170 }, (WIDTH as f32 - 30.0, HEIGHT as f32 - 22.0), SPECIMEN_MARK, Anchor::RightBottom);
    c
}

/// The cat (or given photo) in a room, holding the card tilted in front of it.
pub fn render_selfie(card: &Canvas, look: &CatLook, custom_photo: Option<&RgbaImage>, rng: &mut Rng) -> Canvas {
    let room = CatLook { background: *WALLS.choose(rng).expect("walls"), ..look.clone() };
    let mut selfie = photo(SELFIE_WIDTH, SELFIE_HEIGHT, &room, custom_photo);
    selfie.blur(0.3);
    let scale = rng.random_range(0.5..0.62);
    let angle: f32 = rng.random_range(-8.0..8.0);
    let (w, h) = (card.width() as f32 * scale, card.height() as f32 * scale);
    let (sin, cos) = angle.to_radians().sin_cos();
    let (bw, bh) = (w * cos.abs() + h * sin.abs(), w * sin.abs() + h * cos.abs());
    let x = (SELFIE_WIDTH as f32 - bw) / 2.0 + rng.random_range(-60..=60) as f32;
    let y = SELFIE_HEIGHT as f32 - bh - rng.random_range(30..=120) as f32;
    let transform = Transform::from_translate(x + bw / 2.0, y + bh / 2.0)
        .pre_rotate(angle)
        .pre_scale(scale, scale)
        .pre_translate(-(card.width() as f32) / 2.0, -(card.height() as f32) / 2.0);
    selfie.draw(&card.pixmap, transform, None);
    selfie
}
