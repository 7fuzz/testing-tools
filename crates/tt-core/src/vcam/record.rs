use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread::JoinHandle;
use anyhow::{Context, Result};

/// Pipes composited frames into ffmpeg (H.264 MP4). Frames go through a short queue on a writer
/// thread, so a slow encoder drops frames instead of stalling the camera.
pub struct Recorder {
    pub path: PathBuf,
    pub size: (u32, u32),
    tx: Option<SyncSender<Vec<u8>>>,
    writer: Option<JoinHandle<()>>,
    child: Child,
    pub frames: u64,
    pub dropped: u64,
}

impl Recorder {
    pub fn start(path: &Path, size: (u32, u32), fps: u32) -> Result<Self> {
        let mut child = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba"])
            .args(["-s", &format!("{}x{}", size.0, size.1), "-r", &fps.max(1).to_string(), "-i", "pipe:0"])
            // yuv420p needs even dimensions; pad by a pixel when the canvas is odd.
            .args(["-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2", "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p"])
            .args(["-movflags", "+faststart"])
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("couldn't start ffmpeg; is it installed?")?;
        let mut stdin = child.stdin.take().expect("stdin is piped");
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(3);
        let writer = std::thread::Builder::new()
            .name("vcam-record".into())
            .spawn(move || {
                for frame in rx {
                    if stdin.write_all(&frame).is_err() {
                        break;
                    }
                }
            })
            .context("couldn't start the recording thread")?;
        Ok(Self { path: path.to_path_buf(), size, tx: Some(tx), writer: Some(writer), child, frames: 0, dropped: 0 })
    }

    pub fn push(&mut self, rgba: &[u8]) {
        let Some(tx) = &self.tx else { return };
        match tx.try_send(rgba.to_vec()) {
            Ok(()) => self.frames += 1,
            Err(TrySendError::Full(_)) => self.dropped += 1,
            Err(TrySendError::Disconnected(_)) => self.tx = None,
        }
    }

    /// Closes the input and waits for ffmpeg to finish writing the file.
    pub fn finish(mut self) -> Result<PathBuf> {
        self.tx = None;
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        let status = self.child.wait().context("waiting for ffmpeg")?;
        anyhow::ensure!(status.success(), "ffmpeg exited with {status}");
        Ok(self.path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_a_playable_mp4() {
        if !crate::vcam::video::tests::ffmpeg_available() {
            return;
        }
        let path = std::env::temp_dir().join(format!("tt-record-{}.mp4", std::process::id()));
        let mut recorder = Recorder::start(&path, (63, 40), 10).unwrap();
        let frame = vec![200u8; 63 * 40 * 4];
        for _ in 0..10 {
            recorder.push(&frame);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let path = recorder.finish().unwrap();
        assert_eq!(crate::vcam::video::probe_size(&path).unwrap(), (64, 40));
        let _ = std::fs::remove_file(path);
    }
}
