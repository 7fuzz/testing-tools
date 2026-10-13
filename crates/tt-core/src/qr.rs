use anyhow::{Context, Result, anyhow};
use image::{GrayImage, Luma, RgbaImage};
use qrcode::{Color, EcLevel, QrCode};

/// Renders `data` as a black-on-white QR code: `scale` pixels per module and a `margin`-module quiet zone.
pub fn render(data: &str, scale: u32, margin: u32) -> Result<RgbaImage> {
    let code = QrCode::with_error_correction_level(data, EcLevel::M).context("text is too long for a QR code")?;
    let modules = code.width() as u32;
    let scale = scale.max(1);
    let side = (modules + 2 * margin) * scale;
    let colors = code.to_colors();
    let gray = GrayImage::from_fn(side, side, |x, y| {
        let (mx, my) = ((x / scale).checked_sub(margin), (y / scale).checked_sub(margin));
        let dark = match (mx, my) {
            (Some(mx), Some(my)) if mx < modules && my < modules => colors[(my * modules + mx) as usize] == Color::Dark,
            _ => false,
        };
        Luma([if dark { 0 } else { 255 }])
    });
    Ok(image::DynamicImage::ImageLuma8(gray).to_rgba8())
}

/// Every QR code found in `image`, decoded as text.
pub fn decode_all(image: &image::DynamicImage) -> Vec<String> {
    let gray = image.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(gray.width() as usize, gray.height() as usize, |x, y| {
        gray.get_pixel(x as u32, y as u32).0[0]
    });
    prepared.detect_grids().into_iter().filter_map(|grid| grid.decode().ok().map(|(_, text)| text)).collect()
}

pub fn decode(image: &image::DynamicImage) -> Result<String> {
    decode_all(image).into_iter().next().ok_or_else(|| anyhow!("no QR code found in the image"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_code_decodes_back() {
        let text = "00020101021126670014ID.CO.QRIS.WWW";
        let image = render(text, 6, 4).unwrap();
        assert_eq!(decode(&image::DynamicImage::ImageRgba8(image)).unwrap(), text);
    }

    #[test]
    fn size_follows_scale_and_margin() {
        let image = render("hi", 3, 2).unwrap();
        // "hi" fits a version 1 code: 21 modules.
        assert_eq!(image.width(), (21 + 4) * 3);
        assert_eq!(image.get_pixel(0, 0).0, [255, 255, 255, 255]);
        assert_eq!(image.get_pixel(6, 6).0, [0, 0, 0, 255]);
    }
}
