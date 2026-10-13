//! An OBS-style compositor: a scene of stacked, transformed layers rendered at a fixed rate and
//! written to a v4l2loopback device as a virtual camera.

pub mod compositor;
pub mod convert;
pub mod engine;
pub mod record;
pub mod scene;
pub mod sources;
#[cfg(target_os = "linux")]
pub mod v4l2;
pub mod video;
pub mod webcam;
