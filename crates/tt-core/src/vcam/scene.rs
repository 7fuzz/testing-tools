use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tiny_skia::Transform as Affine;

pub type LayerId = u64;

pub fn new_id() -> LayerId {
    crate::generator::random_seed()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scene {
    pub id: LayerId,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub background: [u8; 3],
    /// Bottom to top.
    pub layers: Vec<Layer>,
}

impl Default for Scene {
    fn default() -> Self {
        Self { id: new_id(), name: "Scene".into(), width: 1280, height: 720, fps: 30, background: [0, 0, 0], layers: Vec::new() }
    }
}

impl Scene {
    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn index_of(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    /// The topmost visible, unlocked layer under `point` (scene pixels).
    pub fn hit(&self, point: (f32, f32)) -> Option<LayerId> {
        self.layers.iter().rev().find(|l| l.visible && !l.locked && l.transform.contains(point)).map(|l| l.id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub source: Source,
    pub visible: bool,
    pub locked: bool,
    pub transform: Transform,
    pub crop: Crop,
    pub opacity: f32,
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            id: new_id(),
            name: String::new(),
            source: Source::Color { rgba: [128, 128, 128, 255] },
            visible: true,
            locked: false,
            transform: Transform::default(),
            crop: Crop::default(),
            opacity: 1.0,
        }
    }
}

impl Layer {
    pub fn new(name: impl Into<String>, source: Source, transform: Transform) -> Self {
        Self { name: name.into(), source, transform, ..Default::default() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    Color { rgba: [u8; 4] },
    TestPattern,
    Image { path: PathBuf },
    Video { path: PathBuf, looping: bool, speed: f32 },
    /// Output of a registered generator; the layer owns the params so the scene reproduces on its own.
    Generated { generator: String, params: Value, seed: u64 },
    Text(TextSource),
    Webcam { device: String },
}

impl Source {
    /// Sources whose pixels change from frame to frame.
    pub fn is_dynamic(&self) -> bool {
        match self {
            Source::Video { .. } | Source::Webcam { .. } => true,
            Source::Text(text) => text.content.contains('{'),
            _ => false,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Source::Color { .. } => "Color",
            Source::TestPattern => "Test pattern",
            Source::Image { .. } => "Image",
            Source::Video { .. } => "Video",
            Source::Generated { .. } => "Generated",
            Source::Text(_) => "Text",
            Source::Webcam { .. } => "Webcam",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextSource {
    /// `{time}` and `{date}` are replaced while rendering.
    pub content: String,
    pub size: f32,
    pub color: [u8; 4],
    pub bold: bool,
    pub outline: bool,
    pub background: Option<[u8; 4]>,
}

impl Default for TextSource {
    fn default() -> Self {
        Self { content: "TEST".into(), size: 48.0, color: [255, 255, 255, 255], bold: true, outline: true, background: None }
    }
}

/// Where a layer sits on the canvas: its box, rotated about the box centre.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rotation: f32,
    pub flip_h: bool,
    pub flip_v: bool,
}

impl Default for Transform {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, w: 100.0, h: 100.0, rotation: 0.0, flip_h: false, flip_v: false }
    }
}

impl Transform {
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Maps the unit box `(0,0)`–`(1,1)` onto the layer's place on the canvas.
    pub fn unit_to_scene(&self) -> Affine {
        let (cx, cy) = self.center();
        let (sx, sy) = (if self.flip_h { -1.0 } else { 1.0 }, if self.flip_v { -1.0 } else { 1.0 });
        Affine::from_translate(cx, cy).pre_rotate(self.rotation).pre_scale(sx * self.w, sy * self.h).pre_translate(-0.5, -0.5)
    }

    /// `point` in the layer's own unrotated box, relative to its centre.
    pub fn to_local(&self, point: (f32, f32)) -> (f32, f32) {
        let (cx, cy) = self.center();
        let (dx, dy) = (point.0 - cx, point.1 - cy);
        let (sin, cos) = (-self.rotation.to_radians()).sin_cos();
        (dx * cos - dy * sin, dx * sin + dy * cos)
    }

    pub fn from_local(&self, local: (f32, f32)) -> (f32, f32) {
        let (cx, cy) = self.center();
        let (sin, cos) = self.rotation.to_radians().sin_cos();
        (cx + local.0 * cos - local.1 * sin, cy + local.0 * sin + local.1 * cos)
    }

    pub fn contains(&self, point: (f32, f32)) -> bool {
        let (lx, ly) = self.to_local(point);
        lx.abs() <= self.w.abs() / 2.0 && ly.abs() <= self.h.abs() / 2.0
    }

    /// The four corners on the canvas: top-left, top-right, bottom-right, bottom-left.
    pub fn corners(&self) -> [(f32, f32); 4] {
        let (hw, hh) = (self.w / 2.0, self.h / 2.0);
        [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].map(|p| self.from_local(p))
    }

    /// Axis-aligned bounds of the rotated box.
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let corners = self.corners();
        let xs = corners.map(|c| c.0);
        let ys = corners.map(|c| c.1);
        let min = |v: [f32; 4]| v.into_iter().fold(f32::INFINITY, f32::min);
        let max = |v: [f32; 4]| v.into_iter().fold(f32::NEG_INFINITY, f32::max);
        (min(xs), min(ys), max(xs), max(ys))
    }

    /// A box of `size` scaled to fit inside `canvas` and centred.
    pub fn fit(size: (f32, f32), canvas: (f32, f32)) -> Self {
        Self::scaled(size, canvas, (canvas.0 / size.0).min(canvas.1 / size.1))
    }

    /// A box of `size` scaled to cover `canvas` and centred.
    pub fn fill(size: (f32, f32), canvas: (f32, f32)) -> Self {
        Self::scaled(size, canvas, (canvas.0 / size.0).max(canvas.1 / size.1))
    }

    fn scaled(size: (f32, f32), canvas: (f32, f32), scale: f32) -> Self {
        let (w, h) = (size.0 * scale, size.1 * scale);
        Self { x: (canvas.0 - w) / 2.0, y: (canvas.1 - h) / 2.0, w, h, ..Default::default() }
    }
}

/// Fractions (0–1) of the source cut from each side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Crop {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Crop {
    pub fn is_none(&self) -> bool {
        *self == Crop::default()
    }

    /// The kept part of a `width`×`height` source, as `(x, y, w, h)` in source pixels.
    pub fn region(&self, width: f32, height: f32) -> (f32, f32, f32, f32) {
        let clamp = |v: f32| v.clamp(0.0, 0.95);
        let (l, t) = (clamp(self.left) * width, clamp(self.top) * height);
        let w = (width - l - clamp(self.right) * width).max(1.0);
        let h = (height - t - clamp(self.bottom) * height).max(1.0);
        (l, t, w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    #[test]
    fn rotated_hit_test() {
        let t = Transform { x: 0.0, y: 0.0, w: 100.0, h: 20.0, rotation: 90.0, ..Default::default() };
        // Rotated 90° about (50, 10): now spans x 40–60, y -40–60.
        assert!(t.contains((50.0, 55.0)));
        assert!(!t.contains((90.0, 10.0)));
        let (x0, y0, x1, y1) = t.bounds();
        assert!(close((x0, y0), (40.0, -40.0)) && close((x1, y1), (60.0, 60.0)));
    }

    #[test]
    fn local_round_trip_and_unit_mapping() {
        let t = Transform { x: 10.0, y: 20.0, w: 40.0, h: 30.0, rotation: 33.0, ..Default::default() };
        let p = (17.0, 41.0);
        assert!(close(t.from_local(t.to_local(p)), p));
        let mut corner = tiny_skia::Point::from_xy(0.0, 0.0);
        t.unit_to_scene().map_points(std::slice::from_mut(&mut corner));
        assert!(close((corner.x, corner.y), t.corners()[0]));
    }

    #[test]
    fn topmost_visible_unlocked_layer_wins() {
        let mut scene = Scene::default();
        let full = Transform { w: 100.0, h: 100.0, ..Default::default() };
        let bottom = Layer::new("bottom", Source::TestPattern, full);
        let mut top = Layer::new("top", Source::TestPattern, full);
        top.locked = true;
        let bottom_id = bottom.id;
        scene.layers = vec![bottom, top];
        assert_eq!(scene.hit((50.0, 50.0)), Some(bottom_id));
        assert_eq!(scene.hit((150.0, 50.0)), None);
    }

    #[test]
    fn fit_and_fill() {
        let fit = Transform::fit((200.0, 100.0), (100.0, 100.0));
        assert!(close((fit.w, fit.h), (100.0, 50.0)) && close((fit.x, fit.y), (0.0, 25.0)));
        let fill = Transform::fill((200.0, 100.0), (100.0, 100.0));
        assert!(close((fill.w, fill.h), (200.0, 100.0)) && close((fill.x, fill.y), (-50.0, 0.0)));
    }

    #[test]
    fn scenes_round_trip_as_json() {
        let mut scene = Scene::default();
        scene.layers.push(Layer::new(
            "qris",
            Source::Generated { generator: "qris".into(), params: serde_json::json!({"name": "TOKO"}), seed: 3 },
            Transform::default(),
        ));
        let json = serde_json::to_string(&scene).unwrap();
        assert_eq!(serde_json::from_str::<Scene>(&json).unwrap(), scene);
    }
}
