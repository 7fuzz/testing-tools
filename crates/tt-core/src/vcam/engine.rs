use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use image::RgbaImage;
use super::compositor::{Compositor, frame_to_rgba};
use super::convert::rgba_to_yuyv;
use super::record::Recorder;
use super::scene::{LayerId, Scene};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Output {
    #[default]
    Off,
    Live {
        device: String,
        frames: u64,
        /// What the device is actually running at.
        size: (u32, u32),
        /// Set when the device is held at another size by a camera app, so frames are scaled to fit.
        held_at: Option<(u32, u32)>,
    },
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Recording {
    #[default]
    Off,
    Active {
        path: PathBuf,
        frames: u64,
        dropped: u64,
    },
    /// The file is being finalised.
    Saving(PathBuf),
    Saved(PathBuf),
    Failed(String),
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub fps: f32,
    /// Compositing plus conversion and write time of the last frame.
    pub frame_ms: f32,
    /// Ticks that took longer than the frame interval.
    pub late: u64,
    pub output: Output,
    pub recording: Recording,
    pub loading: usize,
    pub errors: Vec<(LayerId, String)>,
    pub sizes: HashMap<LayerId, (u32, u32)>,
}

pub struct Frame {
    /// Increases with every new frame, so the UI only re-uploads when it changed.
    pub generation: u64,
    pub image: RgbaImage,
}

enum Command {
    Scene(Box<Scene>),
    Reload(LayerId),
    StartOutput(String),
    StopOutput,
    StartRecording(PathBuf),
    StopRecording,
    Shutdown,
}

#[derive(Default)]
struct Shared {
    preview: Mutex<Option<Arc<Frame>>>,
    stats: Mutex<Stats>,
    /// Set by the thread that finalises a recording, after the render loop has let go of it.
    finished_recording: Mutex<Option<Recording>>,
}

/// The render thread: composites the current scene at its frame rate, publishes a preview, and
/// writes to the virtual camera while output is on.
pub struct Engine {
    tx: mpsc::Sender<Command>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    /// `on_frame` runs on the render thread after each new preview frame, e.g. to request a repaint.
    pub fn start(on_frame: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let thread = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("vcam-render".into())
                .spawn(move || run(&rx, shared, &on_frame))
                .expect("spawn render thread")
        };
        Self { tx, shared, thread: Some(thread) }
    }

    pub fn set_scene(&self, scene: &Scene) {
        let _ = self.tx.send(Command::Scene(Box::new(scene.clone())));
    }

    /// Loads a layer's source again (restarts a video from the beginning).
    pub fn reload(&self, layer: LayerId) {
        let _ = self.tx.send(Command::Reload(layer));
    }

    pub fn start_output(&self, device: &str) {
        let _ = self.tx.send(Command::StartOutput(device.to_string()));
    }

    pub fn stop_output(&self) {
        let _ = self.tx.send(Command::StopOutput);
    }

    /// Records the composited output to an MP4 at `path` until [`Engine::stop_recording`].
    pub fn start_recording(&self, path: &std::path::Path) {
        let _ = self.tx.send(Command::StartRecording(path.to_path_buf()));
    }

    pub fn stop_recording(&self) {
        let _ = self.tx.send(Command::StopRecording);
    }

    pub fn preview(&self) -> Option<Arc<Frame>> {
        self.shared.preview.lock().expect("preview lock").clone()
    }

    pub fn stats(&self) -> Stats {
        self.shared.stats.lock().expect("stats lock").clone()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(target_os = "linux")]
type Device = super::v4l2::OutputDevice;

#[cfg(target_os = "linux")]
fn open_device(path: &str, scene: &Scene) -> anyhow::Result<Device> {
    super::v4l2::OutputDevice::open(path, even(scene.width), scene.height.max(1))
}

#[cfg(not(target_os = "linux"))]
struct Device;

#[cfg(not(target_os = "linux"))]
impl Device {
    fn write_frame(&mut self, _: &[u8]) -> anyhow::Result<()> {
        unreachable!()
    }
}

#[cfg(not(target_os = "linux"))]
fn open_device(_: &str, _: &Scene) -> anyhow::Result<Device> {
    anyhow::bail!("the virtual camera needs Linux and v4l2loopback")
}

/// An open output plus the size it was asked for; the device may be running at another size.
struct Out {
    path: String,
    device: Device,
    requested: (u32, u32),
    retry_at: Instant,
}

const RETRY_SIZE_EVERY: Duration = Duration::from_secs(3);

#[cfg(target_os = "linux")]
fn device_size(device: &Device) -> (u32, u32) {
    (device.width, device.height)
}

#[cfg(not(target_os = "linux"))]
fn device_size(_: &Device) -> (u32, u32) {
    (0, 0)
}

#[cfg(target_os = "linux")]
fn opened_elsewhere(path: &str) -> bool {
    super::v4l2::opened_elsewhere(path)
}

#[cfg(not(target_os = "linux"))]
fn opened_elsewhere(_: &str) -> bool {
    false
}

fn requested_size(scene: &Scene) -> (u32, u32) {
    (even(scene.width), scene.height.max(1))
}

fn live(out: &Out, frames: u64) -> Output {
    let size = device_size(&out.device);
    Output::Live { device: out.path.clone(), frames, size, held_at: (size != out.requested).then_some(size) }
}

/// `frame` letterboxed into `size`, for a device that is held at a size other than the canvas.
fn fit_into(frame: &RgbaImage, size: (u32, u32)) -> RgbaImage {
    let scale = (size.0 as f32 / frame.width() as f32).min(size.1 as f32 / frame.height() as f32);
    let (w, h) = (((frame.width() as f32 * scale) as u32).max(1), ((frame.height() as f32 * scale) as u32).max(1));
    let scaled = image::imageops::resize(frame, w, h, image::imageops::FilterType::Triangle);
    let mut out = RgbaImage::from_pixel(size.0, size.1, image::Rgba([0, 0, 0, 255]));
    image::imageops::overlay(&mut out, &scaled, i64::from((size.0 - w) / 2), i64::from((size.1 - h) / 2));
    out
}

/// YUYV packs pixel pairs, so the output width must be even.
fn even(width: u32) -> u32 {
    (width.max(2) + 1) & !1
}

fn finish_recording(shared: &Arc<Shared>, recorder: Recorder) -> Recording {
    let path = recorder.path.clone();
    let shared = Arc::clone(shared);
    std::thread::spawn(move || {
        let done = match recorder.finish() {
            Ok(path) => Recording::Saved(path),
            Err(error) => Recording::Failed(format!("{error:#}")),
        };
        *shared.finished_recording.lock().expect("recording lock") = Some(done);
    });
    Recording::Saving(path)
}

fn run(rx: &mpsc::Receiver<Command>, shared: Arc<Shared>, on_frame: &dyn Fn()) {
    let shared = &shared;
    let mut scene = Scene::default();
    let mut compositor = Compositor::default();
    let mut out: Option<Out> = None;
    let mut output = Output::Off;
    let mut recorder: Option<Recorder> = None;
    let mut recording = Recording::Off;
    let mut yuyv = Vec::new();
    let mut yuyv_fresh = false;
    let mut generation = 0;
    let mut late = 0;
    let mut window = (Instant::now(), 0u32, 0.0f32);
    let mut next_tick = Instant::now();

    loop {
        let wait = next_tick.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(Command::Scene(new)) => {
                scene = *new;
                continue;
            }
            Ok(Command::Reload(id)) => {
                compositor.reload(id);
                continue;
            }
            Ok(Command::StartOutput(path)) => {
                out = None;
                output = match open_device(&path, &scene) {
                    Ok(device) => {
                        let opened = Out { path, device, requested: requested_size(&scene), retry_at: Instant::now() + RETRY_SIZE_EVERY };
                        let state = live(&opened, 0);
                        out = Some(opened);
                        yuyv_fresh = false;
                        state
                    }
                    Err(error) => Output::Failed(format!("{error:#}")),
                };
                continue;
            }
            Ok(Command::StopOutput) => {
                out = None;
                output = Output::Off;
                continue;
            }
            Ok(Command::StartRecording(path)) => {
                if let Some(old) = recorder.take() {
                    finish_recording(shared, old);
                }
                recording = match Recorder::start(&path, (scene.width.max(1), scene.height.max(1)), scene.fps) {
                    Ok(started) => {
                        recorder = Some(started);
                        Recording::Active { path, frames: 0, dropped: 0 }
                    }
                    Err(error) => Recording::Failed(format!("{error:#}")),
                };
                continue;
            }
            Ok(Command::StopRecording) => {
                if let Some(active) = recorder.take() {
                    recording = finish_recording(shared, active);
                }
                continue;
            }
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }

        let interval = Duration::from_secs_f64(1.0 / f64::from(scene.fps.clamp(1, 120)));
        let started = Instant::now();
        next_tick = (next_tick + interval).max(started);

        // The device format follows the canvas. It can only change size while nothing else has it
        // open, and only once our own handle is closed; closing it while a camera app reads would make
        // the camera vanish for that app. So while held, frames are scaled to the held size and the
        // resize is retried every few seconds until the app lets go.
        if let Some(current) = &mut out {
            let wanted = requested_size(&scene);
            let changed = current.requested != wanted;
            current.requested = wanted;
            let behind = device_size(&current.device) != wanted;
            if behind && (changed || Instant::now() >= current.retry_at) {
                current.retry_at = Instant::now() + RETRY_SIZE_EVERY;
                if !opened_elsewhere(&current.path) {
                    let path = current.path.clone();
                    out = None;
                    match open_device(&path, &scene) {
                        Ok(device) => out = Some(Out { path, device, requested: wanted, retry_at: Instant::now() + RETRY_SIZE_EVERY }),
                        Err(error) => output = Output::Failed(format!("{error:#}")),
                    }
                }
                yuyv_fresh = false;
            }
            if let (Some(current), Output::Live { frames, .. }) = (&out, &output) {
                output = live(current, *frames);
            }
        }

        if let Some(active) = &recorder
            && active.size != (scene.width.max(1), scene.height.max(1))
        {
            // ffmpeg's input size is fixed; changing the canvas ends the recording.
            recording = finish_recording(shared, recorder.take().expect("checked"));
        }

        let composite = compositor.render(&scene);
        if let Some(active) = &mut recorder {
            active.push(composite.frame.data());
            recording = Recording::Active { path: active.path.clone(), frames: active.frames, dropped: active.dropped };
        }
        if composite.changed {
            yuyv_fresh = false;
            generation += 1;
            *shared.preview.lock().expect("preview lock") = Some(Arc::new(Frame { generation, image: frame_to_rgba(composite.frame) }));
            on_frame();
        }
        if let Some(current) = &mut out {
            if !yuyv_fresh {
                let frame = composite.frame;
                let size = device_size(&current.device);
                if (frame.width(), frame.height()) == size {
                    rgba_to_yuyv(frame.data(), size.0, size.1, &mut yuyv);
                } else {
                    let fitted = fit_into(&frame_to_rgba(frame), size);
                    rgba_to_yuyv(fitted.as_raw(), size.0, size.1, &mut yuyv);
                }
                yuyv_fresh = true;
            }
            match current.device.write_frame(&yuyv) {
                Ok(()) => {
                    if let Output::Live { frames, .. } = &mut output {
                        *frames += 1;
                    }
                }
                Err(error) => {
                    output = Output::Failed(format!("{error:#}"));
                    out = None;
                }
            }
        }

        let took = started.elapsed();
        if took > interval {
            late += 1;
        }
        window.1 += 1;
        window.2 = took.as_secs_f32() * 1000.0;
        let elapsed = window.0.elapsed().as_secs_f32();
        let mut stats = shared.stats.lock().expect("stats lock");
        if elapsed >= 1.0 {
            stats.fps = window.1 as f32 / elapsed;
            window = (Instant::now(), 0, window.2);
        }
        stats.frame_ms = window.2;
        stats.late = late;
        stats.output = output.clone();
        if let Some(done) = shared.finished_recording.lock().expect("recording lock").take() {
            recording = done;
        }
        stats.recording = recording.clone();
        stats.loading = compositor.loading();
        stats.errors = compositor.errors();
        stats.sizes = compositor.sizes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcam::scene::{Layer, Source, Transform};
    use std::sync::atomic::{AtomicU32, Ordering};

    fn wait_for(mut done: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn publishes_preview_frames_and_stats() {
        let repaints = Arc::new(AtomicU32::new(0));
        let engine = {
            let repaints = Arc::clone(&repaints);
            Engine::start(move || {
                repaints.fetch_add(1, Ordering::Relaxed);
            })
        };
        let mut scene = Scene { width: 64, height: 32, fps: 60, ..Default::default() };
        scene.layers.push(Layer::new("red", Source::Color { rgba: [255, 0, 0, 255] }, Transform { w: 64.0, h: 32.0, ..Default::default() }));
        engine.set_scene(&scene);
        assert!(wait_for(|| engine.preview().is_some_and(|f| f.image.get_pixel(10, 10).0 == [255, 0, 0, 255])));
        assert!(repaints.load(Ordering::Relaxed) >= 1);
        assert!(wait_for(|| engine.stats().fps > 10.0), "fps {}", engine.stats().fps);

        // A static scene doesn't publish new frames.
        let generation = engine.preview().unwrap().generation;
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(engine.preview().unwrap().generation, generation);
    }

    #[test]
    fn output_to_a_missing_device_fails_cleanly() {
        let engine = Engine::start(|| {});
        engine.set_scene(&Scene { width: 32, height: 16, ..Default::default() });
        engine.start_output("/dev/video-does-not-exist");
        assert!(wait_for(|| matches!(engine.stats().output, Output::Failed(_))));
    }

    #[test]
    fn video_layers_keep_producing_frames() {
        let Some(path) = crate::vcam::video::tests::test_clip("engine", 2) else { return };
        let engine = Engine::start(|| {});
        let mut scene = Scene { width: 64, height: 48, fps: 30, ..Default::default() };
        scene.layers.push(Layer::new(
            "clip",
            Source::Video { path: path.clone(), looping: true, speed: 1.0 },
            Transform { w: 64.0, h: 48.0, ..Default::default() },
        ));
        engine.set_scene(&scene);
        assert!(wait_for(|| engine.preview().is_some_and(|f| f.generation > 5)));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn records_the_output_to_mp4() {
        if !crate::vcam::video::tests::ffmpeg_available() {
            return;
        }
        let engine = Engine::start(|| {});
        let mut scene = Scene { width: 160, height: 90, fps: 30, ..Default::default() };
        scene.layers.push(Layer::new("p", Source::TestPattern, Transform { w: 160.0, h: 90.0, ..Default::default() }));
        engine.set_scene(&scene);
        let path = std::env::temp_dir().join(format!("tt-engine-record-{}.mp4", std::process::id()));
        engine.start_recording(&path);
        assert!(wait_for(|| matches!(engine.stats().recording, Recording::Active { frames, .. } if frames >= 15)));
        engine.stop_recording();
        assert!(wait_for(|| matches!(engine.stats().recording, Recording::Saved(_))), "{:?}", engine.stats().recording);
        assert_eq!(crate::vcam::video::probe_size(&path).unwrap(), (160, 90));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn frames_letterbox_into_a_held_device_size() {
        let frame = RgbaImage::from_pixel(1280, 720, image::Rgba([255, 0, 0, 255]));
        let fitted = fit_into(&frame, (640, 480));
        assert_eq!(fitted.dimensions(), (640, 480));
        assert_eq!(fitted.get_pixel(320, 10).0, [0, 0, 0, 255]);
        assert_eq!(fitted.get_pixel(320, 240).0, [255, 0, 0, 255]);
    }

    #[test]
    fn odd_widths_round_up_to_even() {
        assert_eq!(even(641), 642);
        assert_eq!(even(640), 640);
        assert_eq!(even(0), 2);
    }

    /// Streams to a real v4l2loopback device and reads it back like a camera app would.
    /// `TT_VCAM_DEVICE=/dev/video10 cargo test -p tt-core loopback -- --ignored`
    #[test]
    #[ignore = "needs a v4l2loopback device and ffmpeg"]
    fn loopback_round_trip() {
        let device = std::env::var("TT_VCAM_DEVICE").unwrap_or_else(|_| "/dev/video10".into());
        let engine = Engine::start(|| {});
        let mut scene = Scene { width: 640, height: 480, fps: 30, background: [0, 0, 0], ..Default::default() };
        let half = Transform { w: 320.0, h: 480.0, ..Default::default() };
        scene.layers.push(Layer::new("red", Source::Color { rgba: [220, 20, 20, 255] }, half));
        scene.layers.push(Layer::new("blue", Source::Color { rgba: [20, 20, 220, 255] }, Transform { x: 320.0, ..half }));
        engine.set_scene(&scene);
        engine.start_output(&device);
        assert!(wait_for(|| matches!(engine.stats().output, Output::Live { frames, .. } if frames > 5)), "{:?}", engine.stats().output);

        let out = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-f", "v4l2", "-i", &device, "-frames:v", "3", "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
            .output()
            .expect("ran ffmpeg");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let frame = &out.stdout[out.stdout.len() - 640 * 480 * 3..];
        let at = |x: usize, y: usize| &frame[(y * 640 + x) * 3..(y * 640 + x) * 3 + 3];
        let (left, right) = (at(100, 240), at(540, 240));
        eprintln!("read back: left {left:?}, right {right:?}");
        assert!(left[0] > 180 && left[2] < 60, "left {left:?}");
        assert!(right[2] > 180 && right[0] < 60, "right {right:?}");
        engine.stop_output();
    }

    /// Keeps a test pattern streaming so camera apps can be pointed at the device.
    /// `TT_VCAM_SECONDS=30 cargo test -p tt-core stream_test_pattern -- --ignored`
    #[test]
    #[ignore = "streams to a real v4l2loopback device for a while"]
    fn stream_test_pattern() {
        let device = std::env::var("TT_VCAM_DEVICE").unwrap_or_else(|_| "/dev/video10".into());
        let seconds: u64 = std::env::var("TT_VCAM_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(15);
        let engine = Engine::start(|| {});
        let mut scene = Scene { width: 1280, height: 720, fps: 30, ..Default::default() };
        scene.layers.push(Layer::new("pattern", Source::TestPattern, Transform { w: 1280.0, h: 720.0, ..Default::default() }));
        engine.set_scene(&scene);
        engine.start_output(&device);
        std::thread::sleep(Duration::from_secs(seconds));
        let stats = engine.stats();
        eprintln!("after {seconds}s: {:?}, {:.0} fps", stats.output, stats.fps);
        assert!(matches!(stats.output, Output::Live { .. }));
    }
}
