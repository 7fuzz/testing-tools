use eframe::egui;
use image::RgbaImage;

/// A GPU copy of a generated image, re-uploaded only when its key changes.
#[derive(Default)]
pub struct CachedTexture {
    key: Option<String>,
    handle: Option<egui::TextureHandle>,
}

impl CachedTexture {
    pub fn is_current(&self, key: &str) -> bool {
        self.key.as_deref() == Some(key)
    }

    pub fn set(&mut self, ctx: &egui::Context, key: &str, image: &RgbaImage, options: egui::TextureOptions) {
        let color = egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw());
        match &mut self.handle {
            Some(handle) => handle.set(color, options),
            None => self.handle = Some(ctx.load_texture(key, color, options)),
        }
        self.key = Some(key.to_string());
    }

    pub fn clear(&mut self) {
        self.key = None;
        self.handle = None;
    }

    /// Draws the texture scaled to fit `max`, keeping its aspect ratio.
    pub fn show_fit(&self, ui: &mut egui::Ui, max: egui::Vec2) -> Option<egui::Response> {
        let handle = self.handle.as_ref()?;
        let size = handle.size_vec2();
        let scale = (max.x / size.x).min(max.y / size.y).max(0.01);
        Some(ui.add(egui::Image::new((handle.id(), size * scale))))
    }
}
