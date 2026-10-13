//! The cartoon cat that stands in for a person's photo.

use rand::Rng as _;
use rand::SeedableRng;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use crate::generator::Rng;
use crate::render::{self, Canvas, Rgb};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fur {
    Oren,
    Abu,
    Hitam,
    Putih,
    Krem,
    Cokelat,
}

impl Fur {
    pub const ALL: [Fur; 6] = [Fur::Oren, Fur::Abu, Fur::Hitam, Fur::Putih, Fur::Krem, Fur::Cokelat];

    /// Base colour, then the stripe colour.
    fn colors(self) -> (Rgb, Rgb) {
        match self {
            Fur::Oren => ((235, 160, 80), (200, 115, 45)),
            Fur::Abu => ((150, 150, 160), (95, 95, 105)),
            Fur::Hitam => ((45, 45, 50), (25, 25, 30)),
            Fur::Putih => ((240, 238, 232), (205, 200, 195)),
            Fur::Krem => ((232, 208, 168), (195, 160, 115)),
            Fur::Cokelat => ((140, 95, 60), (90, 58, 35)),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Fur::Oren => "Orange",
            Fur::Abu => "Grey",
            Fur::Hitam => "Black",
            Fur::Putih => "White",
            Fur::Krem => "Cream",
            Fur::Cokelat => "Brown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pattern {
    Polos,
    Belang,
    Tuksedo,
    Calico,
}

const EYES: [Rgb; 5] = [(90, 160, 60), (220, 180, 40), (80, 150, 220), (200, 130, 40), (140, 190, 90)];
const PHOTO_BACKGROUNDS: [Rgb; 4] = [(196, 40, 40), (180, 30, 45), (35, 85, 190), (30, 110, 200)];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatLook {
    pub background: Rgb,
    pub fur: Fur,
    pub pattern: Pattern,
    pub eyes: (Rgb, Rgb),
    /// Head radius as a share of the image width.
    pub head: f32,
    pub ear: f32,
    pub eye: f32,
    pub pupil: f32,
    pub whiskers: u32,
    /// Seeds the stripe and patch placement.
    pub coat_seed: u64,
}

impl CatLook {
    pub fn random(rng: &mut Rng) -> Self {
        let fur = *Fur::ALL.choose(rng).expect("furs");
        let mut patterns = vec![Pattern::Polos, Pattern::Belang, Pattern::Tuksedo];
        if fur == Fur::Putih {
            patterns.extend([Pattern::Calico, Pattern::Calico]);
        }
        let eye = *EYES.choose(rng).expect("eyes");
        let other = if rng.random_bool(0.1) { *EYES.choose(rng).expect("eyes") } else { eye };
        Self {
            background: *PHOTO_BACKGROUNDS.choose(rng).expect("backgrounds"),
            fur,
            pattern: *patterns.choose(rng).expect("patterns"),
            eyes: (eye, other),
            head: rng.random_range(0.31..0.40),
            ear: rng.random_range(0.75..1.3),
            eye: rng.random_range(0.8..1.3),
            pupil: rng.random_range(0.2..0.75),
            whiskers: rng.random_range(2..=4),
            coat_seed: rng.random(),
        }
    }
}

fn coat(width: u32, height: u32, look: &CatLook, cx: f32, cy: f32, r: f32, s: f32) -> Canvas {
    let (base, stripe) = look.fur.colors();
    let mut c = Canvas::new(width, height, base);
    let mut rng = Rng::seed_from_u64(look.coat_seed);
    let h = height as f32;
    match look.pattern {
        Pattern::Polos => {}
        Pattern::Belang => {
            for k in [-1.0, 0.0, 1.0] {
                let x = cx + k * r * 0.22;
                let end = cy - r * rng.random_range(0.4..0.55);
                c.line(&[(x, cy - r), (x + k * r * 0.05, end)], (8.0 * s).floor(), stripe, false);
            }
            for side in [-1.0, 1.0] {
                for j in 0..rng.random_range(2..=3) {
                    let y = cy + r * (0.05 + j as f32 * 0.2) + rng.random_range(-4.0..4.0) * s;
                    c.line(&[(cx + side * r, y), (cx + side * r * 0.62, y + 3.0 * s)], (6.0 * s).floor(), stripe, false);
                }
            }
            for j in 0..4 {
                let y = h - 60.0 * s + j as f32 * 22.0 * s;
                c.arc((cx - r - 30.0 * s, y, cx + r + 30.0 * s, y + 80.0 * s), 200.0, 340.0, (7.0 * s).floor(), stripe);
            }
        }
        Pattern::Calico => {
            for _ in 0..rng.random_range(3..=5) {
                let px = cx + rng.random_range(-1.2..1.2) * r;
                let py = cy + rng.random_range(-1.3..0.9) * r;
                let pr = r * rng.random_range(0.25..0.55);
                let color = if rng.random_bool(0.5) { (235, 150, 70) } else { (55, 48, 45) };
                c.fill_ellipse(px - pr, py - pr * 0.8, px + pr, py + pr * 0.8, color);
            }
            c.fill_ellipse(cx - r * 0.45, cy + r * 0.1, cx + r * 0.45, cy + r * 0.85, base);
        }
        Pattern::Tuksedo => {
            let white = if look.fur == Fur::Putih { stripe } else { (245, 245, 240) };
            c.fill_ellipse(cx - r * 0.5, cy + r * 0.05, cx + r * 0.5, cy + r * 0.9, white);
            c.fill_polygon(&[(cx, cy - r * 0.35), (cx - r * 0.18, cy + r * 0.2), (cx + r * 0.18, cy + r * 0.2)], white);
            c.fill_ellipse(cx - r * 0.55, h - 55.0 * s, cx + r * 0.55, h + 80.0 * s, white);
        }
    }
    c
}

/// A cat portrait filling `width`×`height`, scaled from ktp_cat.py's 210-pixel-wide original.
pub fn draw(width: u32, height: u32, look: &CatLook) -> Canvas {
    let (w, h) = (width as f32, height as f32);
    let s = w / 210.0;
    let mut img = Canvas::new(width, height, look.background);
    let (cx, cy, r) = ((w / 2.0).floor(), (h * 0.55).floor(), (w * look.head).floor());
    let (ear_h, tip) = (45.0 * s * look.ear, 10.0 * s);

    let mut shapes = Vec::new();
    shapes.extend(render::oval(cx - r - 20.0 * s, h - 70.0 * s, cx + r + 20.0 * s, h + 120.0 * s));
    for sx in [-1.0, 1.0] {
        shapes.extend(render::polygon(&[
            (cx + sx * r, cy - r / 3.0),
            (cx + sx * r / 2.0, cy - r - ear_h),
            (cx + sx * r / 6.0, cy - r + tip),
        ]));
    }
    shapes.extend(render::oval(cx - r, cy - r, cx + r, cy + r));
    let mask = render::mask(width, height, &shapes);
    let fur = coat(width, height, look, cx, cy, r, s);
    img.draw(&fur.pixmap, tiny_skia::Transform::identity(), Some(&mask));

    let dark = (60, 40, 30);
    let line = if look.fur == Fur::Hitam { (200, 200, 200) } else { dark };
    for sx in [-1.0, 1.0] {
        img.fill_polygon(
            &[
                (cx + sx * (r - 18.0 * s), cy - r / 3.0 - 8.0 * s),
                (cx + sx * r / 2.0, cy - r - ear_h * 0.5),
                (cx + sx * r / 4.0, cy - r + 18.0 * s),
            ],
            (245, 190, 190),
        );
    }
    for (sx, color) in [(-1.0, look.eyes.0), (1.0, look.eyes.1)] {
        let (ex, ey) = (cx + sx * r / 2.0, cy - 4.0 * s);
        let (rx, ry) = (16.0 * s * look.eye, 18.0 * s * look.eye);
        img.fill_ellipse(ex - rx, ey - ry, ex + rx, ey + ry, color);
        let px = rx * look.pupil;
        img.fill_ellipse(ex - px, ey - ry * 0.9, ex + px, ey + ry * 0.9, (25, 20, 15));
        img.fill_ellipse(ex - rx * 0.45, ey - ry * 0.6, ex - rx * 0.15, ey - ry * 0.3, (255, 255, 255));
    }
    img.fill_polygon(&[(cx - 9.0 * s, cy + 22.0 * s), (cx + 9.0 * s, cy + 22.0 * s), (cx, cy + 32.0 * s)], (220, 110, 120));
    let mouth = (3.0 * s).floor().max(2.0);
    img.arc((cx - 18.0 * s, cy + 26.0 * s, cx, cy + 46.0 * s), 0.0, 180.0, mouth, line);
    img.arc((cx, cy + 26.0 * s, cx + 18.0 * s, cy + 46.0 * s), 0.0, 180.0, mouth, line);
    let spread = 24.0 * s / (look.whiskers.max(2) - 1) as f32;
    let whisker = (2.0 * s).floor().max(1.0);
    for i in 0..look.whiskers {
        let dy = -6.0 * s + i as f32 * spread;
        img.line(&[(cx - 22.0 * s, cy + 30.0 * s), (cx - r - 15.0 * s, cy + 30.0 * s + dy)], whisker, line, false);
        img.line(&[(cx + 22.0 * s, cy + 30.0 * s), (cx + r + 15.0 * s, cy + 30.0 * s + dy)], whisker, line, false);
    }
    img
}
