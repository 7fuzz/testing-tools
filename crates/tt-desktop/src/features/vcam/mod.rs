mod add;
mod canvas;
mod history;
mod layers;
mod properties;
mod scenes;
mod view;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use eframe::egui;
use serde::{Deserialize, Serialize};
use tt_core::vcam::engine::Engine;
use tt_core::vcam::scene::{Layer, LayerId, Scene, Source, Transform};
use tt_core::vcam::v4l2::DeviceInfo;
use crate::util::job::Job;
pub use view::show;

pub const RESOLUTIONS: [(u32, u32); 4] = [(640, 480), (1280, 720), (1920, 1080), (720, 1280)];
pub const FRAME_RATES: [u32; 3] = [15, 30, 60];

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct VcamPrefs {
    pub scenes: Vec<Scene>,
    pub active: usize,
    pub device: Option<String>,
}

impl Default for VcamPrefs {
    fn default() -> Self {
        Self { scenes: vec![starter_scene()], active: 0, device: None }
    }
}

fn starter_scene() -> Scene {
    let mut scene = Scene { name: "Scene 1".into(), ..Default::default() };
    let full = Transform { w: scene.width as f32, h: scene.height as f32, ..Default::default() };
    scene.layers.push(Layer::new("Test pattern", Source::TestPattern, full));
    scene
}

impl VcamPrefs {
    pub fn scene(&self) -> &Scene {
        &self.scenes[self.active.min(self.scenes.len() - 1)]
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        let index = self.active.min(self.scenes.len() - 1);
        &mut self.scenes[index]
    }
}

/// The params key a draft render was made for, and the image size or the error.
type DraftRender = (String, Result<(u32, u32), String>);

/// A generator layer being set up in the quick-create popup.
pub struct Draft {
    pub generator: &'static str,
    pub params: serde_json::Value,
    pub seed: u64,
    preview: crate::util::texture::CachedTexture,
    rendering: Option<(String, Job<Result<image::RgbaImage, String>>)>,
    rendered: Option<DraftRender>,
}

#[derive(Default)]
pub struct VcamState {
    pub prefs: VcamPrefs,
    engine: Option<Engine>,
    /// When the tab was last drawn (ms since start); the render thread only requests repaints while it's visible.
    shown_at: Arc<AtomicU64>,
    preview: Option<(u64, egui::TextureHandle)>,
    devices: Option<Vec<DeviceInfo>>,
    pub selected: Option<LayerId>,
    sent: Option<Scene>,
    history: history::History,
    canvas: canvas::CanvasState,
    draft: Option<Draft>,
    pending_add: Option<Job<(Vec<Layer>, Vec<String>)>>,
    renaming: Option<(LayerId, crate::widgets::rename::Rename)>,
    renaming_scene: Option<(usize, crate::widgets::rename::Rename)>,
    last_recording: Option<tt_core::vcam::engine::Recording>,
}

fn now_ms(ctx: &egui::Context) -> u64 {
    (ctx.input(|i| i.time) * 1000.0) as u64
}

impl VcamState {
    fn engine(&mut self, ctx: &egui::Context) -> &Engine {
        let shown_at = Arc::clone(&self.shown_at);
        let ctx = ctx.clone();
        self.engine.get_or_insert_with(move || {
            let started = std::time::Instant::now();
            let base = now_ms(&ctx);
            Engine::start(move || {
                let now = base + started.elapsed().as_millis() as u64;
                if now.saturating_sub(shown_at.load(Ordering::Relaxed)) < 1000 {
                    ctx.request_repaint();
                }
            })
        })
    }

    fn devices(&mut self) -> &[DeviceInfo] {
        self.devices.get_or_insert_with(tt_core::vcam::v4l2::scan)
    }

    pub fn output_devices(&mut self) -> Vec<DeviceInfo> {
        self.devices().iter().filter(|d| d.is_output).cloned().collect()
    }

    pub fn webcams(&mut self) -> Vec<DeviceInfo> {
        self.devices().iter().filter(|d| d.is_capture && !d.is_loopback()).cloned().collect()
    }

    /// Opens quick create with the given params, for "Send to camera" from a generator tab.
    pub fn prefill(&mut self, generator: &str, params: serde_json::Value, seed: u64) {
        if let Some(entry) = crate::generators::find(generator) {
            self.draft = Some(Draft { params, seed, ..Draft::new(entry) });
        }
    }

    pub fn add_layer(&mut self, layer: Layer) {
        self.selected = Some(layer.id);
        self.prefs.scene_mut().layers.push(layer);
    }
}

#[cfg(test)]
pub fn screenshots() {
    use crate::app::state::AppState;
    use crate::screenshots::shoot_tab;
    use crate::ui::dock::Tab;
    shoot_tab("vcam", Tab::Vcam, AppState::default(), |state| {
        let scene = state.vcam.prefs.scene_mut();
        let params = tt_core::generator::to_json(&tt_core::qris::Merchant::default());
        let mut qris = Layer::new(
            "QRIS",
            Source::Generated { generator: "qris".into(), params, seed: 0 },
            Transform { x: 820.0, y: 220.0, w: 340.0, h: 340.0, rotation: -6.0, ..Default::default() },
        );
        qris.opacity = 0.95;
        state.vcam.selected = Some(qris.id);
        scene.layers.push(qris);
        state.vcam.devices = Some(Vec::new());
    });
    shoot_tab("vcam-quick-create", Tab::Vcam, AppState::default(), |state| {
        state.vcam.devices = Some(Vec::new());
        let params = tt_core::generator::to_json(&tt_core::qris::Merchant { dynamic: true, amount: Some(25_000), ..Default::default() });
        state.vcam.prefill("qris", params, 1);
    });
}
