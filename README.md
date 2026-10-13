# Testing Tools

A desktop toolbox (Rust, egui) for QA and staging test data:

- **QRIS**: EMVCo QRIS payloads for fictional merchants; inspect, fix CRC, decode QR images, check on staging.
- **KTP specimen**: KTP-layout test cards and selfies with a cartoon cat, always marked SPECIMEN.
- **Virtual camera**: an OBS-style compositor (images, videos, text, webcam, generated QRIS/KTP layers)
  streamed to a v4l2loopback device, with snapshot and MP4 recording.

## Run

```bash
cargo run -p tt-desktop
```

## Install

```bash
packaging/linux/install.sh
```

Installs `testing-tools` into `~/.local/bin`, plus a launcher entry and icons. Preferences and scenes are
saved in `~/.local/share/testing-tools/`.

## Virtual camera setup

The virtual camera needs the `v4l2loopback` kernel module:

```bash
sudo modprobe v4l2loopback devices=1 video_nr=10 card_label="Testing Tools" exclusive_caps=1
```

`exclusive_caps=1` is what Chrome needs to list the device; it then only shows up as a camera while
Testing Tools is streaming, so press **Start camera** before opening the app's camera picker.
To load it on every boot:

```bash
echo v4l2loopback | sudo tee /etc/modules-load.d/v4l2loopback.conf
echo 'options v4l2loopback devices=1 video_nr=10 card_label="Testing Tools" exclusive_caps=1' | sudo tee /etc/modprobe.d/v4l2loopback.conf
```

Video, webcam and recording use `ffmpeg` / `ffprobe` from `PATH`.

## Tests

```bash
cargo test
cargo test -p tt-desktop screenshots -- --ignored        # offscreen tab renders → target/screenshots
TT_VCAM_DEVICE=/dev/video10 cargo test -p tt-core loopback -- --ignored   # stream to a real loopback device
```
