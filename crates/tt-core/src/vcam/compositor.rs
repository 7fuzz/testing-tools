use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::mpsc;
use image::RgbaImage;
use tiny_skia::{FilterQuality, Paint, Pattern, Pixmap, Rect, SpreadMode, Transform as Affine};
use super::scene::{Layer, LayerId, Scene, Source};
use super::sources;

/// Big photos are shrunk on load; bilinear sampling of a huge source into a small box aliases badly.
const MAX_SOURCE_SIDE: u32 = 2560;

enum Slot {
    Loading,
    Ready(Pixmap),
    Live(Box<dyn sources::LiveSource>, Option<Pixmap>),
    Failed(String),
}

struct Entry {
    key: u64,
    slot: Slot,
    /// Bumped whenever the pixels change, so cached runs know to redraw.
    revision: u64,
}

enum Loaded {
    Image(Pixmap),
    Live(Box<dyn sources::LiveSource>),
    Failed(String),
}

/// A stack of consecutive static layers flattened into one transparent pixmap.
struct Run {
    key: u64,
    pixmap: Pixmap,
}

/// An upright layer's source, already cropped, flipped and resized to its box.
struct Scaled {
    key: u64,
    pixmap: Pixmap,
}

pub struct Compositor {
    entries: HashMap<LayerId, Entry>,
    loads_tx: mpsc::Sender<(LayerId, u64, Loaded)>,
    loads_rx: mpsc::Receiver<(LayerId, u64, Loaded)>,
    runs: Vec<Run>,
    /// Run keys drawn last frame; a run is only cached once it has stayed the same for a frame,
    /// so dragging a layer doesn't rebuild a cache every frame.
    last_run_keys: Vec<u64>,
    scaled: HashMap<LayerId, Scaled>,
    frame: Option<Pixmap>,
    frame_key: Option<u64>,
}

impl Default for Compositor {
    fn default() -> Self {
        let (loads_tx, loads_rx) = mpsc::channel();
        Self {
            entries: HashMap::new(),
            loads_tx,
            loads_rx,
            runs: Vec::new(),
            last_run_keys: Vec::new(),
            scaled: HashMap::new(),
            frame: None,
            frame_key: None,
        }
    }
}

fn hash_of(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn source_key(source: &Source, scene: &Scene) -> u64 {
    let json = serde_json::to_string(source).unwrap_or_default();
    // Test patterns and live sources depend on the canvas size or rate as well.
    hash_of(&(json, scene.width, scene.height, scene.fps))
}

fn layer_key(layer: &Layer, revision: u64) -> u64 {
    hash_of(&(serde_json::to_string(layer).unwrap_or_default(), revision))
}

pub fn pixmap_from_rgba(image: &RgbaImage) -> Pixmap {
    let image = shrink(image);
    let mut pixmap = Pixmap::new(image.width().max(1), image.height().max(1)).expect("non-empty");
    for (dst, src) in pixmap.pixels_mut().iter_mut().zip(image.pixels()) {
        let [r, g, b, a] = src.0;
        *dst = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    pixmap
}

fn shrink(image: &RgbaImage) -> std::borrow::Cow<'_, RgbaImage> {
    let longest = image.width().max(image.height());
    if longest <= MAX_SOURCE_SIDE {
        return std::borrow::Cow::Borrowed(image);
    }
    let scale = MAX_SOURCE_SIDE as f32 / longest as f32;
    let (w, h) = ((image.width() as f32 * scale) as u32, (image.height() as f32 * scale) as u32);
    std::borrow::Cow::Owned(image::imageops::resize(image, w.max(1), h.max(1), image::imageops::FilterType::Triangle))
}

/// Draws `source` into the layer's box, honouring crop, rotation, flips and opacity.
pub fn draw_layer(target: &mut Pixmap, layer: &Layer, source: &Pixmap) {
    let t = &layer.transform;
    if t.w.abs() < 0.5 || t.h.abs() < 0.5 || layer.opacity <= 0.0 {
        return;
    }
    let (cl, ct, cw, ch) = layer.crop.region(source.width() as f32, source.height() as f32);
    let (cx, cy) = t.center();
    let (sx, sy) = (if t.flip_h { -1.0 } else { 1.0 }, if t.flip_v { -1.0 } else { 1.0 });
    let local = Affine::from_translate(cx, cy).pre_rotate(t.rotation).pre_scale(sx * t.w / cw, sy * t.h / ch).pre_translate(-cw / 2.0, -ch / 2.0);
    let shader = Pattern::new(source.as_ref(), SpreadMode::Pad, FilterQuality::Bilinear, layer.opacity.min(1.0), Affine::from_translate(-cl, -ct));
    let paint = Paint { anti_alias: true, shader, ..Default::default() };
    if let Some(rect) = Rect::from_xywh(0.0, 0.0, cw, ch) {
        target.fill_rect(rect, &paint, local, None);
    }
}

/// Premultiplied source-over of `src` (the `rect` part of it) onto `dst` at `(dx, dy)`.
pub fn blit_over(dst: &mut Pixmap, src: &Pixmap, rect: (u32, u32, u32, u32), dx: i32, dy: i32, opacity: f32) {
    let (sx, sy, sw, sh) = rect;
    let (dw, dh) = (dst.width() as i32, dst.height() as i32);
    let x0 = dx.max(0);
    let y0 = dy.max(0);
    let x1 = (dx + sw as i32).min(dw);
    let y1 = (dy + sh as i32).min(dh);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
    let src_stride = src.width() as usize * 4;
    let dst_stride = dst.width() as usize * 4;
    let src_data = src.data();
    let dst_data = dst.data_mut();
    let span = (x1 - x0) as usize * 4;
    for y in y0..y1 {
        let s_row = (sy as i32 + y - dy) as usize * src_stride + (sx as i32 + x0 - dx) as usize * 4;
        let d_row = y as usize * dst_stride + x0 as usize * 4;
        let s = &src_data[s_row..s_row + span];
        let d = &mut dst_data[d_row..d_row + span];
        for (sp, dp) in s.as_chunks::<4>().0.iter().zip(d.as_chunks_mut::<4>().0.iter_mut()) {
            let sa = u32::from(sp[3]) * alpha / 255;
            if sa == 0 {
                continue;
            }
            if sa == 255 {
                dp.copy_from_slice(sp);
                continue;
            }
            let inv = 255 - sa;
            for i in 0..4 {
                let sc = u32::from(sp[i]) * alpha / 255;
                dp[i] = (sc + (u32::from(dp[i]) * inv + 127) / 255).min(255) as u8;
            }
        }
    }
}

fn is_upright(layer: &Layer) -> bool {
    let turn = layer.transform.rotation.rem_euclid(360.0);
    turn < 0.01 || turn > 359.99
}

/// The cropped source resized to `size`, flipped as the layer asks.
fn prepare_scaled(source: &Pixmap, layer: &Layer, size: (u32, u32)) -> Pixmap {
    let (cl, ct, cw, ch) = layer.crop.region(source.width() as f32, source.height() as f32);
    let full = RgbaImage::from_raw(source.width(), source.height(), source.data().to_vec()).expect("pixmap size matches");
    let cropped = image::imageops::crop_imm(&full, cl as u32, ct as u32, (cw as u32).max(1), (ch as u32).max(1)).to_image();
    let mut resized = if cropped.dimensions() == size {
        cropped
    } else {
        image::imageops::resize(&cropped, size.0, size.1, image::imageops::FilterType::Triangle)
    };
    if layer.transform.flip_h {
        image::imageops::flip_horizontal_in_place(&mut resized);
    }
    if layer.transform.flip_v {
        image::imageops::flip_vertical_in_place(&mut resized);
    }
    let mut pixmap = Pixmap::new(size.0, size.1).expect("non-empty");
    pixmap.data_mut().copy_from_slice(resized.as_raw());
    pixmap
}

pub struct Composite<'a> {
    pub frame: &'a Pixmap,
    /// False when the frame is identical to the previous call's.
    pub changed: bool,
}

impl Compositor {
    fn sync(&mut self, scene: &Scene) {
        self.entries.retain(|id, _| scene.layer(*id).is_some());
        for layer in &scene.layers {
            let key = source_key(&layer.source, scene);
            if self.entries.get(&layer.id).is_some_and(|e| e.key == key) {
                continue;
            }
            let revision = self.entries.get(&layer.id).map_or(0, |e| e.revision + 1);
            let slot = match sources::immediate(&layer.source, scene) {
                Some(Ok(image)) => Slot::Ready(pixmap_from_rgba(&image)),
                Some(Err(error)) => Slot::Failed(error),
                None => {
                    let (tx, id, source, (fps, size)) = (self.loads_tx.clone(), layer.id, layer.source.clone(), (scene.fps, (scene.width, scene.height)));
                    std::thread::spawn(move || {
                        let loaded = match sources::load(&source, fps, size) {
                            Ok(sources::Loaded::Image(image)) => Loaded::Image(pixmap_from_rgba(&image)),
                            Ok(sources::Loaded::Live(live)) => Loaded::Live(live),
                            Err(error) => Loaded::Failed(error),
                        };
                        let _ = tx.send((id, key, loaded));
                    });
                    Slot::Loading
                }
            };
            self.entries.insert(layer.id, Entry { key, slot, revision });
        }
    }

    fn collect_loads(&mut self) {
        while let Ok((id, key, loaded)) = self.loads_rx.try_recv() {
            let Some(entry) = self.entries.get_mut(&id).filter(|e| e.key == key) else { continue };
            entry.slot = match loaded {
                Loaded::Image(pixmap) => Slot::Ready(pixmap),
                Loaded::Live(live) => Slot::Live(live, None),
                Loaded::Failed(error) => Slot::Failed(error),
            };
            entry.revision += 1;
        }
    }

    fn pull_live_frames(&mut self) {
        for entry in self.entries.values_mut() {
            if let Slot::Live(live, frame) = &mut entry.slot {
                if let Some(image) = live.take_frame() {
                    *frame = Some(pixmap_from_rgba(&image));
                    entry.revision += 1;
                } else if let Some(error) = live.error() {
                    entry.slot = Slot::Failed(error);
                    entry.revision += 1;
                }
            }
        }
    }

    fn pixmap(&self, id: LayerId) -> Option<&Pixmap> {
        match &self.entries.get(&id)?.slot {
            Slot::Ready(pixmap) => Some(pixmap),
            Slot::Live(_, frame) => frame.as_ref(),
            Slot::Loading | Slot::Failed(_) => None,
        }
    }

    /// Drops a layer's loaded source so it loads again, e.g. to restart a video.
    pub fn reload(&mut self, id: LayerId) {
        self.entries.remove(&id);
        self.scaled.remove(&id);
    }

    /// Layers still loading.
    pub fn loading(&self) -> usize {
        self.entries.values().filter(|e| matches!(e.slot, Slot::Loading)).count()
    }

    pub fn errors(&self) -> Vec<(LayerId, String)> {
        self.entries
            .iter()
            .filter_map(|(id, e)| match &e.slot {
                Slot::Failed(error) => Some((*id, error.clone())),
                _ => None,
            })
            .collect()
    }

    /// Natural pixel size of each loaded source.
    pub fn sizes(&self) -> HashMap<LayerId, (u32, u32)> {
        self.entries.keys().filter_map(|id| self.pixmap(*id).map(|p| (*id, (p.width(), p.height())))).collect()
    }

    fn draw(&mut self, target: &mut Pixmap, layer: &Layer) {
        let Some(entry) = self.entries.get(&layer.id) else { return };
        let source = match &entry.slot {
            Slot::Ready(pixmap) => pixmap,
            Slot::Live(_, Some(frame)) => frame,
            _ => return,
        };
        let t = &layer.transform;
        if !is_upright(layer) || t.w < 0.5 || t.h < 0.5 {
            draw_layer(target, layer, source);
            return;
        }
        let size = (t.w.round() as u32, t.h.round() as u32);
        let (x, y) = (t.x.round() as i32, t.y.round() as i32);
        let unscaled = layer.crop.is_none() && !t.flip_h && !t.flip_v && size == (source.width(), source.height());
        if unscaled {
            blit_over(target, source, (0, 0, size.0, size.1), x, y, layer.opacity);
            return;
        }
        let crop = [layer.crop.left, layer.crop.top, layer.crop.right, layer.crop.bottom].map(f32::to_bits);
        let key = hash_of(&(entry.revision, crop, size, t.flip_h, t.flip_v));
        if self.scaled.get(&layer.id).is_none_or(|s| s.key != key) {
            let pixmap = prepare_scaled(source, layer, size);
            self.scaled.insert(layer.id, Scaled { key, pixmap });
        }
        let scaled = &self.scaled[&layer.id].pixmap;
        blit_over(target, scaled, (0, 0, size.0, size.1), x, y, layer.opacity);
    }

    pub fn render(&mut self, scene: &Scene) -> Composite<'_> {
        self.sync(scene);
        self.collect_loads();
        self.pull_live_frames();

        let visible: Vec<&Layer> = scene.layers.iter().filter(|l| l.visible).collect();
        let revisions: HashMap<LayerId, u64> = self.entries.iter().map(|(id, e)| (*id, e.revision)).collect();
        let revision = |id: LayerId| revisions.get(&id).copied().unwrap_or(0);
        let live: Vec<LayerId> = self.entries.iter().filter(|(_, e)| matches!(e.slot, Slot::Live(..))).map(|(id, _)| *id).collect();
        let dynamic = |layer: &Layer| live.contains(&layer.id);
        let frame_key = hash_of(&(
            scene.width,
            scene.height,
            scene.background,
            visible.iter().map(|l| layer_key(l, revision(l.id))).collect::<Vec<_>>(),
        ));
        let size_ok = self.frame.as_ref().is_some_and(|f| f.width() == scene.width && f.height() == scene.height);
        if size_ok && self.frame_key == Some(frame_key) {
            return Composite { frame: self.frame.as_ref().expect("checked"), changed: false };
        }

        // Group consecutive static layers; each group renders once into a cached pixmap.
        let mut groups: Vec<(bool, Vec<&Layer>)> = Vec::new();
        for layer in visible {
            let live = dynamic(layer);
            match groups.last_mut() {
                Some((false, run)) if !live => run.push(layer),
                _ => groups.push((live, vec![layer])),
            }
        }

        let (w, h) = (scene.width.max(2), scene.height.max(1));
        let mut frame = match self.frame.take() {
            Some(frame) if frame.width() == w && frame.height() == h => frame,
            _ => Pixmap::new(w, h).expect("non-empty"),
        };
        let [r, g, b] = scene.background;
        frame.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));

        self.scaled.retain(|id, _| scene.layer(*id).is_some());
        let mut runs = std::mem::take(&mut self.runs);
        let mut used = Vec::new();
        let mut run_keys = Vec::new();
        for (live, layers) in &groups {
            if *live {
                for layer in layers {
                    self.draw(&mut frame, layer);
                }
                continue;
            }
            let key = hash_of(&(w, h, layers.iter().map(|l| layer_key(l, revision(l.id))).collect::<Vec<_>>()));
            run_keys.push(key);
            let cached = runs.iter().position(|r| r.key == key).map(|index| runs.swap_remove(index));
            let run = match cached {
                Some(run) => run,
                None if self.last_run_keys.contains(&key) => {
                    let mut pixmap = Pixmap::new(w, h).expect("non-empty");
                    for layer in layers {
                        self.draw(&mut pixmap, layer);
                    }
                    Run { key, pixmap }
                }
                None => {
                    for layer in layers {
                        self.draw(&mut frame, layer);
                    }
                    continue;
                }
            };
            blit_over(&mut frame, &run.pixmap, (0, 0, w, h), 0, 0, 1.0);
            used.push(run);
        }
        self.runs = used;
        self.last_run_keys = run_keys;
        self.frame = Some(frame);
        self.frame_key = Some(frame_key);
        Composite { frame: self.frame.as_ref().expect("just set"), changed: true }
    }
}

/// An opaque frame as straight RGBA.
pub fn frame_to_rgba(frame: &Pixmap) -> RgbaImage {
    RgbaImage::from_raw(frame.width(), frame.height(), frame.data().to_vec()).expect("pixmap size matches")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcam::scene::Transform;
    use std::time::{Duration, Instant};

    fn scene(layers: Vec<Layer>) -> Scene {
        Scene { width: 100, height: 60, background: [0, 0, 0], layers, ..Default::default() }
    }

    fn color(rgba: [u8; 4], t: Transform) -> Layer {
        Layer::new("c", Source::Color { rgba }, t)
    }

    fn pixel(frame: &Pixmap, x: u32, y: u32) -> [u8; 4] {
        let c = frame.pixel(x, y).unwrap();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }

    #[test]
    fn layers_stack_bottom_to_top() {
        let mut compositor = Compositor::default();
        let s = scene(vec![
            color([255, 0, 0, 255], Transform { x: 0.0, y: 0.0, w: 60.0, h: 60.0, ..Default::default() }),
            color([0, 0, 255, 255], Transform { x: 40.0, y: 0.0, w: 60.0, h: 60.0, ..Default::default() }),
        ]);
        let out = compositor.render(&s);
        assert!(out.changed);
        assert_eq!(pixel(out.frame, 10, 30), [255, 0, 0, 255]);
        assert_eq!(pixel(out.frame, 50, 30), [0, 0, 255, 255]);
        assert_eq!(pixel(out.frame, 99, 30), [0, 0, 255, 255]);
        assert!(!compositor.render(&s).changed);
    }

    #[test]
    fn hidden_layers_opacity_and_rotation() {
        let mut compositor = Compositor::default();
        let mut hidden = color([0, 255, 0, 255], Transform { w: 100.0, h: 60.0, ..Default::default() });
        hidden.visible = false;
        let mut half = color([255, 255, 255, 255], Transform { w: 20.0, h: 20.0, ..Default::default() });
        half.opacity = 0.5;
        let rotated = color([255, 0, 0, 255], Transform { x: 40.0, y: 20.0, w: 40.0, h: 4.0, rotation: 90.0, ..Default::default() });
        let out = compositor.render(&scene(vec![hidden, half, rotated]));
        assert_eq!(pixel(out.frame, 50, 50)[1], 0);
        let gray = pixel(out.frame, 10, 10)[0];
        assert!((120..=135).contains(&gray), "{gray}");
        // Vertical after rotation: covers x 58–62, y 2–42.
        assert_eq!(pixel(out.frame, 60, 5), [255, 0, 0, 255]);
        assert_eq!(pixel(out.frame, 70, 22), [0, 0, 0, 255]);
    }

    #[test]
    fn crop_shows_part_of_the_source() {
        let mut compositor = Compositor::default();
        let mut layer = Layer::new("pattern", Source::TestPattern, Transform { w: 100.0, h: 60.0, ..Default::default() });
        layer.crop.left = 0.5;
        let s = scene(vec![layer]);
        let full = compositor.render(&scene(vec![Layer::new("p", Source::TestPattern, Transform { w: 100.0, h: 60.0, ..Default::default() })])).frame.clone();
        let cropped = compositor.render(&s).frame.clone();
        assert_eq!(pixel(&cropped, 1, 30), pixel(&full, 51, 30));
    }

    #[test]
    fn generated_layers_load_in_the_background() {
        let mut compositor = Compositor::default();
        let params = crate::generator::to_json(&crate::qris::Merchant::default());
        let layer = Layer::new(
            "qris",
            Source::Generated { generator: "qris".into(), params, seed: 0 },
            Transform { w: 60.0, h: 60.0, ..Default::default() },
        );
        let id = layer.id;
        let s = scene(vec![layer]);
        compositor.render(&s);
        let deadline = Instant::now() + Duration::from_secs(5);
        while compositor.loading() > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            compositor.render(&s);
        }
        assert!(compositor.sizes().contains_key(&id));
        assert_eq!(pixel(compositor.render(&s).frame, 1, 1), [255, 255, 255, 255]);
    }

    #[test]
    fn bad_sources_report_errors() {
        let mut compositor = Compositor::default();
        let s = scene(vec![Layer::new("x", Source::Image { path: "/nope/missing.png".into() }, Transform::default())]);
        compositor.render(&s);
        let deadline = Instant::now() + Duration::from_secs(5);
        while compositor.errors().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            compositor.render(&s);
        }
        assert_eq!(compositor.errors().len(), 1);
    }

    fn timing_scene(rotation: f32) -> Scene {
        let mut s = Scene { width: 1280, height: 720, ..Default::default() };
        s.layers.push(Layer::new("bg", Source::TestPattern, Transform { w: 1280.0, h: 720.0, ..Default::default() }));
        for i in 0..4 {
            let t = Transform { x: 100.0 * i as f32, y: 80.0, w: 500.0, h: 320.0, rotation, ..Default::default() };
            s.layers.push(color([200, 50 * i as u8, 30, 255], t));
        }
        s
    }

    #[test]
    #[ignore = "timing; run with --release"]
    fn upright_moving_layer_at_720p() {
        let mut compositor = Compositor::default();
        let mut s = timing_scene(0.0);
        let mut yuyv = Vec::new();
        let started = Instant::now();
        for i in 0..120 {
            s.layers[4].transform.x = i as f32;
            let out = compositor.render(&s);
            crate::vcam::convert::rgba_to_yuyv(out.frame.data(), 1280, 720, &mut yuyv);
        }
        let per_frame = started.elapsed().as_secs_f32() * 1000.0 / 120.0;
        eprintln!("upright, one layer moving: {per_frame:.2} ms per 720p frame");
        assert!(per_frame < 16.0);
    }

    #[test]
    #[ignore = "timing; run with --release"]
    fn worst_case_frame_time_at_720p() {
        let mut compositor = Compositor::default();
        let mut s = timing_scene(7.0);
        let mut yuyv = Vec::new();
        let frames = 120;
        let started = Instant::now();
        for i in 0..frames {
            // Moving the top layer every frame defeats every cache: the full stack redraws.
            s.layers[4].transform.x = i as f32;
            let out = compositor.render(&s);
            crate::vcam::convert::rgba_to_yuyv(out.frame.data(), 1280, 720, &mut yuyv);
        }
        let per_frame = started.elapsed().as_secs_f32() * 1000.0 / frames as f32;
        eprintln!("worst case: {per_frame:.2} ms per 720p frame (budget 33 ms at 30 fps)");
        assert!(per_frame < 33.0);
    }
}
