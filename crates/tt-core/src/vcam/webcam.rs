use std::process::Command;
use anyhow::{Result, bail};
use image::RgbaImage;
use super::sources::LiveSource;
use super::video::{self, VideoPlayer};

/// A real camera, captured through ffmpeg so MJPEG and YUYV cameras both work.
pub struct Webcam {
    player: VideoPlayer,
}

impl Webcam {
    pub fn open(device: &str) -> Result<Self> {
        #[cfg(target_os = "linux")]
        if let Ok(info) = super::v4l2::query(device)
            && (info.is_loopback() || !info.is_capture)
        {
            bail!("{device} ({}) is a virtual camera; capturing it would feed the output back into itself", info.label);
        }
        let size = video::probe(&["-f", "v4l2"], device.as_ref())?;
        if size.0 == 0 || size.1 == 0 {
            bail!("{device} reported no frame size");
        }
        let mut command = Command::new("ffmpeg");
        command.args(["-v", "error", "-nostdin", "-f", "v4l2", "-i", device, "-an", "-pix_fmt", "rgba", "-f", "rawvideo", "pipe:1"]);
        Ok(Self { player: VideoPlayer::from_command(command, size)? })
    }
}

impl LiveSource for Webcam {
    fn take_frame(&mut self) -> Option<RgbaImage> {
        self.player.take_frame()
    }

    fn error(&self) -> Option<String> {
        self.player.has_ended().then(|| "the camera stopped sending frames (unplugged, or in use by another app?)".to_string())
    }
}
