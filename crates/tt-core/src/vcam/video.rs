use std::io::Read;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use anyhow::{Context, Result, bail};
use image::RgbaImage;

pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "m4v", "gif"];

pub fn is_video_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| VIDEO_EXTENSIONS.iter().any(|v| v.eq_ignore_ascii_case(e)))
}

pub fn probe_size(path: &Path) -> Result<(u32, u32)> {
    probe(&[], path.as_os_str())
}

/// Width and height of the first video stream; `input_args` go before the input (such as `-f v4l2`).
pub fn probe(input_args: &[&str], input: &std::ffi::OsStr) -> Result<(u32, u32)> {
    let out = Command::new("ffprobe")
        .args(["-v", "error"])
        .args(input_args)
        .args(["-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=s=x:p=0"])
        .arg(input)
        .output()
        .context("couldn't run ffprobe; is ffmpeg installed?")?;
    if !out.status.success() {
        bail!("ffprobe failed on {}: {}", input.to_string_lossy(), String::from_utf8_lossy(&out.stderr).trim());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next().unwrap_or_default().trim().trim_end_matches('x');
    let (w, h) = first.split_once('x').with_context(|| format!("unexpected ffprobe output {first:?}"))?;
    Ok((w.parse().context("bad width from ffprobe")?, h.parse().context("bad height from ffprobe")?))
}

/// Decodes a video with ffmpeg in real time, keeping only the newest frame: the compositor samples
/// it at its own rate, so older frames are dropped rather than queued.
pub struct VideoPlayer {
    child: Child,
    latest: Arc<Mutex<Option<RgbaImage>>>,
    frames: Arc<AtomicU64>,
    finished: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
    pub size: (u32, u32),
}

impl VideoPlayer {
    pub fn start(path: &Path, fps: u32, looping: bool, speed: f32) -> Result<Self> {
        let size = probe_size(path)?;
        let speed = if speed.is_finite() && speed > 0.0 { speed.clamp(0.1, 8.0) } else { 1.0 };
        let mut command = Command::new("ffmpeg");
        command.args(["-v", "error", "-nostdin"]);
        if looping {
            command.args(["-stream_loop", "-1"]);
        }
        // -readrate paces decoding against the wall clock (1 = real time), which is how speed is applied.
        // Frames must match the probed size, so rotated phone clips stay sideways; rotate the layer instead.
        command
            .args(["-readrate", &speed.to_string(), "-noautorotate"])
            .arg("-i")
            .arg(path)
            .args(["-an", "-vf", &format!("fps={}", fps.max(1))])
            .args(["-pix_fmt", "rgba", "-f", "rawvideo", "pipe:1"]);
        Self::from_command(command, size)
    }

    /// Runs an ffmpeg `command` that writes raw RGBA frames of `size` to stdout.
    pub fn from_command(mut command: Command, size: (u32, u32)) -> Result<Self> {
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = command.spawn().context("couldn't start ffmpeg; is it installed?")?;
        let stdout = child.stdout.take().expect("stdout is piped");
        let latest = Arc::new(Mutex::new(None));
        let frames = Arc::new(AtomicU64::new(0));
        let finished = Arc::new(AtomicBool::new(false));
        let reader = {
            let (latest, frames, finished) = (Arc::clone(&latest), Arc::clone(&frames), Arc::clone(&finished));
            std::thread::Builder::new()
                .name("vcam-video".into())
                .spawn(move || read_frames(stdout, size, &latest, &frames, &finished))
                .context("couldn't start the decode thread")?
        };
        Ok(Self { child, latest, frames, finished, reader: Some(reader), size })
    }

    /// The newest decoded frame, if one arrived since the last call.
    pub fn take_frame(&self) -> Option<RgbaImage> {
        self.latest.lock().expect("video frame lock").take()
    }

    pub fn frames_decoded(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }

    pub fn has_ended(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn read_frames(mut stdout: ChildStdout, (w, h): (u32, u32), latest: &Mutex<Option<RgbaImage>>, frames: &AtomicU64, finished: &AtomicBool) {
    let mut buffer = vec![0u8; (w * h * 4) as usize];
    while stdout.read_exact(&mut buffer).is_ok() {
        if let Some(frame) = RgbaImage::from_raw(w, h, buffer.clone()) {
            *latest.lock().expect("video frame lock") = Some(frame);
            frames.fetch_add(1, Ordering::Relaxed);
        }
    }
    finished.store(true, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    pub fn ffmpeg_available() -> bool {
        Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success())
    }

    /// A small test clip, or `None` when ffmpeg isn't installed.
    pub fn test_clip(name: &str, seconds: u32) -> Option<std::path::PathBuf> {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed");
            return None;
        }
        let path = std::env::temp_dir().join(format!("tt-vcam-{name}-{}.mp4", std::process::id()));
        let status = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
            .arg(format!("testsrc=size=64x48:rate=10:duration={seconds}"))
            .args(["-pix_fmt", "yuv420p"])
            .arg(&path)
            .status()
            .expect("ran ffmpeg");
        assert!(status.success());
        Some(path)
    }

    #[test]
    fn plays_once_then_ends() {
        let Some(path) = test_clip("once", 1) else { return };
        let player = VideoPlayer::start(&path, 10, false, 2.0).unwrap();
        assert_eq!(player.size, (64, 48));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !player.has_ended() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(30));
        }
        assert!(player.has_ended());
        assert!(player.frames_decoded() > 0);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn loops_without_ending() {
        let Some(path) = test_clip("loop", 1) else { return };
        let player = VideoPlayer::start(&path, 10, true, 4.0).unwrap();
        std::thread::sleep(Duration::from_millis(1200));
        assert!(!player.has_ended());
        assert!(player.frames_decoded() > 10, "decoded {}", player.frames_decoded());
        let _ = std::fs::remove_file(path);
    }
}
