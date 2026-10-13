use std::sync::LazyLock;
use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont};
use super::{Canvas, Rgb};

pub static REGULAR: LazyLock<FontRef<'static>> =
    LazyLock::new(|| FontRef::try_from_slice(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf")).expect("bundled font"));
pub static BOLD: LazyLock<FontRef<'static>> =
    LazyLock::new(|| FontRef::try_from_slice(include_bytes!("../../assets/fonts/NotoSans-Bold.ttf")).expect("bundled font"));

/// Where `(x, y)` sits on the text, as PIL's anchors: horizontal left/middle/right, then
/// ascender line, top of the ink, or bottom of the ink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    LeftAscender,
    MiddleTop,
    RightBottom,
}

/// `size` is the em size in pixels, like PIL's `truetype(size)`.
fn scale(font: &FontRef<'static>, size: f32) -> PxScale {
    PxScale::from(size * font.height_unscaled() / font.units_per_em().unwrap_or(1000.0))
}

struct Laid {
    glyphs: Vec<ab_glyph::Glyph>,
    width: f32,
}

fn lay_out(font: &FontRef<'static>, size: f32, text: &str) -> Laid {
    let scaled = font.as_scaled(scale(font, size));
    let mut glyphs = Vec::new();
    let mut x = 0.0;
    let mut previous: Option<GlyphId> = None;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        if let Some(previous) = previous {
            x += scaled.kern(previous, id);
        }
        glyphs.push(id.with_scale_and_position(scaled.scale(), ab_glyph::point(x, 0.0)));
        x += scaled.h_advance(id);
        previous = Some(id);
    }
    Laid { glyphs, width: x }
}

pub fn width(font: &FontRef<'static>, size: f32, text: &str) -> f32 {
    lay_out(font, size, text).width
}

/// The largest size from `size` down to `min` at which `text` fits in `max_width`.
pub fn fit_size(font: &FontRef<'static>, size: f32, min: f32, text: &str, max_width: f32) -> f32 {
    let mut size = size;
    while size > min && width(font, size, text) > max_width {
        size -= 1.0;
    }
    size
}

/// Font, size in pixels (em, like PIL's `truetype(size)`), colour and opacity.
#[derive(Clone, Copy)]
pub struct Pen {
    pub font: &'static FontRef<'static>,
    pub size: f32,
    pub color: Rgb,
    pub alpha: u8,
}

impl Pen {
    pub fn new(font: &'static FontRef<'static>, size: f32, color: Rgb) -> Self {
        Self { font, size, color, alpha: 255 }
    }
}

pub fn draw(canvas: &mut Canvas, pen: Pen, at: (f32, f32), text: &str, anchor: Anchor) {
    let Pen { font, size, color, alpha } = pen;
    let laid = lay_out(font, size, text);
    let outlines: Vec<_> = laid.glyphs.into_iter().filter_map(|g| font.outline_glyph(g)).collect();
    let ink_top = outlines.iter().map(|o| o.px_bounds().min.y).fold(0.0, f32::min);
    let ink_bottom = outlines.iter().map(|o| o.px_bounds().max.y).fold(0.0, f32::max);
    let (dx, baseline) = match anchor {
        Anchor::LeftAscender => (0.0, at.1 + font.as_scaled(scale(font, size)).ascent()),
        Anchor::MiddleTop => (-laid.width / 2.0, at.1 - ink_top),
        Anchor::RightBottom => (-laid.width, at.1 - ink_bottom),
    };
    let (ox, oy) = (at.0 + dx, baseline);
    let (w, h) = (canvas.width() as i32, canvas.height() as i32);
    let data = canvas.pixmap.data_mut();
    for outline in outlines {
        let bounds = outline.px_bounds();
        outline.draw(|gx, gy, coverage| {
            let x = (ox + bounds.min.x).round() as i32 + gx as i32;
            let y = (oy + bounds.min.y).round() as i32 + gy as i32;
            if x < 0 || y < 0 || x >= w || y >= h {
                return;
            }
            let a = coverage.clamp(0.0, 1.0) * f32::from(alpha) / 255.0;
            let i = ((y * w + x) * 4) as usize;
            // Source-over on premultiplied pixels.
            let src = [f32::from(color.0) * a, f32::from(color.1) * a, f32::from(color.2) * a, 255.0 * a];
            for (channel, s) in data[i..i + 4].iter_mut().zip(src) {
                *channel = (s + f32::from(*channel) * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wider_text_measures_wider_and_fit_shrinks() {
        assert!(width(&REGULAR, 20.0, "WWWW") > width(&REGULAR, 20.0, "iiii"));
        let fitted = fit_size(&REGULAR, 40.0, 12.0, "A LONG NAME THAT DOES NOT FIT", 200.0);
        assert!(fitted < 40.0 && width(&REGULAR, fitted, "A LONG NAME THAT DOES NOT FIT") <= 200.0);
    }

    #[test]
    fn draws_ink_near_the_anchor() {
        let mut canvas = Canvas::new(200, 60, (255, 255, 255));
        draw(&mut canvas, Pen::new(&BOLD, 30.0, (0, 0, 0)), (100.0, 10.0), "HI", Anchor::MiddleTop);
        let image = canvas.to_rgba();
        let dark: Vec<(u32, u32)> = image.enumerate_pixels().filter(|(_, _, p)| p.0[0] < 100).map(|(x, y, _)| (x, y)).collect();
        assert!(!dark.is_empty());
        let min_y = dark.iter().map(|p| p.1).min().unwrap();
        let (min_x, max_x) = (dark.iter().map(|p| p.0).min().unwrap(), dark.iter().map(|p| p.0).max().unwrap());
        assert!((9..=12).contains(&min_y), "ink starts at {min_y}");
        assert!((min_x + max_x) / 2 >= 95 && (min_x + max_x) / 2 <= 105);
    }
}
