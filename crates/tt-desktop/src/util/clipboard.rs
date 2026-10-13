use std::borrow::Cow;
use image::RgbaImage;

pub fn copy_image(image: &RgbaImage) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let data = arboard::ImageData {
        width: image.width() as usize,
        height: image.height() as usize,
        bytes: Cow::Borrowed(image.as_raw()),
    };
    clipboard.set_image(data).map_err(|e| e.to_string())
}

pub fn paste_image() -> Result<RgbaImage, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let data = clipboard.get_image().map_err(|_| "No image on the clipboard".to_string())?;
    RgbaImage::from_raw(data.width as u32, data.height as u32, data.bytes.into_owned())
        .ok_or_else(|| "The clipboard image has an unexpected size".to_string())
}
