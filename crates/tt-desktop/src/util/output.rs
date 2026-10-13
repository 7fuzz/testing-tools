use std::path::{Path, PathBuf};
use image::RgbaImage;

/// `<prefix>-<YYYYmmdd-HHMMSS>-<4 hex>.<ext>`, so files from one batch sort together and never collide.
pub fn file_name(prefix: &str, ext: &str) -> String {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let salt = tt_core::generator::random_seed() & 0xFFFF;
    format!("{prefix}-{stamp}-{salt:04x}.{ext}")
}

pub fn save_png_in(dir: &Path, prefix: &str, image: &RgbaImage) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    let path = dir.join(file_name(prefix, "png"));
    image.save(&path).map_err(|e| format!("Couldn't save {}: {e}", path.display()))?;
    Ok(path)
}

/// Asks where to save, starting in `dir`. `Ok(None)` when the dialog is cancelled.
pub fn save_png_as(dir: &Path, prefix: &str, image: &RgbaImage) -> Result<Option<PathBuf>, String> {
    let Some(path) = rfd::FileDialog::new()
        .set_directory(dir)
        .set_file_name(file_name(prefix, "png"))
        .add_filter("PNG image", &["png"])
        .save_file()
    else {
        return Ok(None);
    };
    image.save(&path).map_err(|e| format!("Couldn't save {}: {e}", path.display()))?;
    Ok(Some(path))
}

pub fn pick_image() -> Option<PathBuf> {
    rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"]).pick_file()
}
