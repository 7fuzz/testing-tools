//! Writing frames to a v4l2loopback output device, via the two ioctls that needs.
#![allow(non_camel_case_types)]

use std::fs::{File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use anyhow::{Context, Result, bail};

const V4L2_BUF_TYPE_VIDEO_OUTPUT: u32 = 2;
const V4L2_FIELD_NONE: u32 = 1;
const V4L2_COLORSPACE_SRGB: u32 = 8;
const V4L2_CAP_VIDEO_CAPTURE: u32 = 0x0000_0001;
const V4L2_CAP_VIDEO_OUTPUT: u32 = 0x0000_0002;
const V4L2_CAP_DEVICE_CAPS: u32 = 0x8000_0000;

const fn fourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

pub const PIX_FMT_YUYV: u32 = fourcc(b'Y', b'U', b'Y', b'V');

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct v4l2_capability {
    driver: [u8; 16],
    card: [u8; 32],
    bus_info: [u8; 32],
    version: u32,
    capabilities: u32,
    device_caps: u32,
    reserved: [u32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct v4l2_pix_format {
    width: u32,
    height: u32,
    pixelformat: u32,
    field: u32,
    bytesperline: u32,
    sizeimage: u32,
    colorspace: u32,
    priv_field: u32,
    flags: u32,
    ycbcr_enc: u32,
    quantization: u32,
    xfer_func: u32,
}

#[repr(C, align(8))]
union v4l2_format_union {
    pix: v4l2_pix_format,
    raw_data: [u8; 200],
}

#[repr(C)]
struct v4l2_format {
    type_: u32,
    _pad: u32,
    fmt: v4l2_format_union,
}

// From linux/videodev2.h for 64-bit targets.
const VIDIOC_QUERYCAP: libc::c_ulong = 0x8068_5600;
const VIDIOC_S_FMT: libc::c_ulong = 0xc0d0_5605;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub path: String,
    pub label: String,
    pub driver: String,
    /// Accepts frames (a loopback output), as opposed to a real camera.
    pub is_output: bool,
    pub is_capture: bool,
}

impl DeviceInfo {
    pub fn is_loopback(&self) -> bool {
        self.driver == "v4l2 loopback"
    }
}

fn c_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim_matches(char::from(0)).to_string()
}

pub fn query(path: &str) -> Result<DeviceInfo> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .or_else(|_| OpenOptions::new().read(true).custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK).open(path))
        .with_context(|| format!("couldn't open {path}"))?;
    let mut cap = v4l2_capability::default();
    // SAFETY: VIDIOC_QUERYCAP fills a v4l2_capability, which this struct mirrors.
    if unsafe { libc::ioctl(file.as_raw_fd(), VIDIOC_QUERYCAP, &mut cap) } < 0 {
        bail!("VIDIOC_QUERYCAP failed on {path}: {}", std::io::Error::last_os_error());
    }
    let caps = if cap.capabilities & V4L2_CAP_DEVICE_CAPS != 0 { cap.device_caps } else { cap.capabilities };
    Ok(DeviceInfo {
        path: path.to_string(),
        label: c_string(&cap.card),
        driver: c_string(&cap.driver),
        is_output: caps & V4L2_CAP_VIDEO_OUTPUT != 0,
        is_capture: caps & V4L2_CAP_VIDEO_CAPTURE != 0,
    })
}

/// `/dev/video*` devices, loopback outputs first, then by number.
pub fn scan() -> Vec<DeviceInfo> {
    let mut devices: Vec<DeviceInfo> = std::fs::read_dir("/dev")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("video"))
        .filter_map(|name| query(&format!("/dev/{name}")).ok())
        .collect();
    let number = |d: &DeviceInfo| d.path.trim_start_matches("/dev/video").parse::<u32>().unwrap_or(u32::MAX);
    devices.sort_by(|a, b| b.is_output.cmp(&a.is_output).then_with(|| number(a).cmp(&number(b))));
    devices
}

pub struct OutputDevice {
    file: File,
    path: String,
    pub width: u32,
    pub height: u32,
    frame_size: usize,
}

impl OutputDevice {
    pub fn open(path: &str, width: u32, height: u32) -> Result<Self> {
        if let Ok(info) = query(path)
            && !info.is_output
        {
            bail!("{path} ({}) is a camera, not a virtual camera output; pick a v4l2loopback device", info.label);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_CLOEXEC)
            .open(path)
            .with_context(|| format!("couldn't open {path} for writing"))?;
        let mut device = Self { file, path: path.to_string(), width, height, frame_size: 0 };
        device.set_format(width, height)?;
        Ok(device)
    }

    fn set_format(&mut self, width: u32, height: u32) -> Result<()> {
        let bytesperline = width * 2;
        let sizeimage = bytesperline * height;
        let mut format = v4l2_format {
            type_: V4L2_BUF_TYPE_VIDEO_OUTPUT,
            _pad: 0,
            fmt: v4l2_format_union {
                pix: v4l2_pix_format {
                    width,
                    height,
                    pixelformat: PIX_FMT_YUYV,
                    field: V4L2_FIELD_NONE,
                    bytesperline,
                    sizeimage,
                    colorspace: V4L2_COLORSPACE_SRGB,
                    priv_field: 0,
                    flags: 0,
                    ycbcr_enc: 0,
                    quantization: 0,
                    xfer_func: 0,
                },
            },
        };
        // SAFETY: VIDIOC_S_FMT reads and updates a v4l2_format, which this struct mirrors.
        if unsafe { libc::ioctl(self.file.as_raw_fd(), VIDIOC_S_FMT, &mut format) } < 0 {
            bail!(
                "couldn't set {width}x{height} YUYV on {}: {} (another app may be holding it at a different size)",
                self.path,
                std::io::Error::last_os_error()
            );
        }
        self.frame_size = sizeimage as usize;
        Ok(())
    }

    pub fn write_frame(&mut self, yuyv: &[u8]) -> Result<()> {
        use std::io::Write;
        if yuyv.len() != self.frame_size {
            bail!("frame is {} bytes, the device expects {}", yuyv.len(), self.frame_size);
        }
        self.file.write_all(yuyv).with_context(|| format!("writing to {}", self.path))
    }
}

pub const LOOPBACK_HINT: &str =
    "sudo modprobe v4l2loopback devices=1 video_nr=10 card_label=\"Testing Tools\" exclusive_caps=1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_layouts_match_the_kernel() {
        assert_eq!(std::mem::size_of::<v4l2_capability>(), 104);
        assert_eq!(std::mem::size_of::<v4l2_pix_format>(), 48);
        assert_eq!(std::mem::size_of::<v4l2_format>(), 208);
    }
}
