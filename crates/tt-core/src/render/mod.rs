//! A small drawing layer over tiny-skia: the shapes the generators need, plus text and image conversion.

pub mod text;

use image::RgbaImage;
use tiny_skia::{FillRule, LineCap, LineJoin, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

pub type Rgb = (u8, u8, u8);

pub struct Canvas {
    pub pixmap: Pixmap,
}

fn paint(color: Rgb, alpha: u8) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.0, color.1, color.2, alpha);
    paint.anti_alias = true;
    paint
}

impl Canvas {
    pub fn new(width: u32, height: u32, fill: Rgb) -> Self {
        let mut pixmap = Pixmap::new(width.max(1), height.max(1)).expect("canvas size is positive");
        pixmap.fill(tiny_skia::Color::from_rgba8(fill.0, fill.1, fill.2, 255));
        Self { pixmap }
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: Rgb) {
        let i = ((y * self.width() + x) * 4) as usize;
        self.pixmap.data_mut()[i..i + 4].copy_from_slice(&[color.0, color.1, color.2, 255]);
    }

    /// The ellipse inside the box `(x0, y0)`–`(x1, y1)`.
    pub fn fill_ellipse(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: Rgb) {
        if let Some(path) = Rect::from_ltrb(x0, y0, x1, y1).and_then(PathBuilder::from_oval) {
            self.pixmap.fill_path(&path, &paint(color, 255), FillRule::Winding, Transform::identity(), None);
        }
    }

    pub fn fill_polygon(&mut self, points: &[(f32, f32)], color: Rgb) {
        if let Some(path) = polygon(points) {
            self.pixmap.fill_path(&path, &paint(color, 255), FillRule::Winding, Transform::identity(), None);
        }
    }

    pub fn fill_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: Rgb, alpha: u8) {
        if let Some(rect) = Rect::from_ltrb(x0, y0, x1, y1) {
            self.pixmap.fill_rect(rect, &paint(color, alpha), Transform::identity(), None);
        }
    }

    pub fn stroke_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: Rgb) {
        let inset = width / 2.0;
        let corners = [(x0 + inset, y0 + inset), (x1 - inset, y0 + inset), (x1 - inset, y1 - inset), (x0 + inset, y1 - inset)];
        if let Some(path) = polygon(&corners) {
            let stroke = Stroke { width, line_join: LineJoin::Miter, ..Default::default() };
            self.pixmap.stroke_path(&path, &paint(color, 255), &stroke, Transform::identity(), None);
        }
    }

    /// A polyline; `round` gives round joins and caps (a pen stroke), otherwise flat ends like PIL lines.
    pub fn line(&mut self, points: &[(f32, f32)], width: f32, color: Rgb, round: bool) {
        let Some(path) = polyline(points) else { return };
        let stroke = Stroke {
            width,
            line_cap: if round { LineCap::Round } else { LineCap::Butt },
            line_join: if round { LineJoin::Round } else { LineJoin::Miter },
            ..Default::default()
        };
        self.pixmap.stroke_path(&path, &paint(color, 255), &stroke, Transform::identity(), None);
    }

    /// Part of the ellipse in the given box, from `start` to `end` degrees, clockwise from 3 o'clock (PIL's `arc`).
    pub fn arc(&mut self, bbox: (f32, f32, f32, f32), start: f32, end: f32, width: f32, color: Rgb) {
        let (x0, y0, x1, y1) = bbox;
        let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0, (y1 - y0) / 2.0);
        let steps = 32;
        let points: Vec<(f32, f32)> = (0..=steps)
            .map(|i| {
                let angle = (start + (end - start) * i as f32 / steps as f32).to_radians();
                (cx + rx * angle.cos(), cy + ry * angle.sin())
            })
            .collect();
        self.line(&points, width, color, false);
    }

    pub fn draw(&mut self, image: &Pixmap, transform: Transform, mask: Option<&Mask>) {
        let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
        self.pixmap.draw_pixmap(0, 0, image.as_ref(), &paint, transform, mask);
    }

    pub fn to_rgba(&self) -> RgbaImage {
        let mut out = RgbaImage::new(self.width(), self.height());
        for (dst, src) in out.pixels_mut().zip(self.pixmap.pixels()) {
            let c = src.demultiply();
            dst.0 = [c.red(), c.green(), c.blue(), c.alpha()];
        }
        out
    }

    pub fn from_rgba(image: &RgbaImage) -> Self {
        let mut pixmap = Pixmap::new(image.width().max(1), image.height().max(1)).expect("image size is positive");
        for (dst, src) in pixmap.pixels_mut().iter_mut().zip(image.pixels()) {
            let [r, g, b, a] = src.0;
            *dst = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
        }
        Self { pixmap }
    }

    pub fn blur(&mut self, sigma: f32) {
        if sigma > 0.0 {
            *self = Self::from_rgba(&image::imageops::blur(&self.to_rgba(), sigma));
        }
    }
}

pub fn polygon(points: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    let mut builder = path_through(points)?;
    builder.close();
    builder.finish()
}

fn polyline(points: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    path_through(points)?.finish()
}

fn path_through(points: &[(f32, f32)]) -> Option<PathBuilder> {
    let (&(x, y), rest) = points.split_first()?;
    let mut builder = PathBuilder::new();
    builder.move_to(x, y);
    for &(x, y) in rest {
        builder.line_to(x, y);
    }
    Some(builder)
}

/// A mask covering the union of `shapes`.
pub fn mask(width: u32, height: u32, shapes: &[tiny_skia::Path]) -> Mask {
    let mut mask = Mask::new(width, height).expect("mask size is positive");
    for shape in shapes {
        mask.fill_path(shape, FillRule::Winding, true, Transform::identity());
    }
    mask
}

pub fn oval(x0: f32, y0: f32, x1: f32, y1: f32) -> Option<tiny_skia::Path> {
    Rect::from_ltrb(x0, y0, x1, y1).and_then(PathBuilder::from_oval)
}

/// `image` scaled to cover `width`×`height` and center-cropped.
pub fn cover(image: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let scale = (width as f32 / image.width() as f32).max(height as f32 / image.height() as f32);
    let (sw, sh) = ((image.width() as f32 * scale).ceil() as u32 + 1, (image.height() as f32 * scale).ceil() as u32 + 1);
    let resized = image::imageops::resize(image, sw, sh, image::imageops::FilterType::Triangle);
    image::imageops::crop_imm(&resized, (sw - width) / 2, (sh - height) / 2, width, height).to_image()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_land_where_asked() {
        let mut canvas = Canvas::new(40, 40, (255, 255, 255));
        canvas.fill_ellipse(10.0, 10.0, 30.0, 30.0, (255, 0, 0));
        canvas.line(&[(0.0, 2.0), (40.0, 2.0)], 2.0, (0, 0, 255), false);
        let image = canvas.to_rgba();
        assert_eq!(image.get_pixel(20, 20).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(2, 20).0, [255, 255, 255, 255]);
        assert_eq!(image.get_pixel(20, 2).0, [0, 0, 255, 255]);
    }

    #[test]
    fn rgba_round_trip_keeps_opaque_pixels() {
        let mut image = RgbaImage::new(3, 1);
        image.put_pixel(1, 0, image::Rgba([10, 20, 30, 255]));
        assert_eq!(Canvas::from_rgba(&image).to_rgba().get_pixel(1, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn cover_fills_and_crops() {
        let image = RgbaImage::new(100, 50);
        assert_eq!(cover(&image, 30, 40).dimensions(), (30, 40));
    }
}
