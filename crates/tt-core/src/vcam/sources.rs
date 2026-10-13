use image::{Rgba, RgbaImage};
use super::scene::{Scene, Source, TextSource};
use super::video::VideoPlayer;
use crate::render::Canvas;
use crate::render::text::{self, Anchor, BOLD, Pen, REGULAR};

/// A source that keeps producing frames (video, webcam, clock text).
pub trait LiveSource: Send {
    /// The newest frame, if there is one since the last call.
    fn take_frame(&mut self) -> Option<RgbaImage>;
    /// Set once the source has failed for good.
    fn error(&self) -> Option<String> {
        None
    }
}

pub enum Loaded {
    Image(RgbaImage),
    Live(Box<dyn LiveSource>),
}

/// Sources cheap enough to build on the render thread; `None` means load in the background.
pub fn immediate(source: &Source, scene: &Scene) -> Option<Result<RgbaImage, String>> {
    match source {
        Source::Color { rgba } => Some(Ok(RgbaImage::from_pixel(1, 1, Rgba(*rgba)))),
        Source::TestPattern => Some(Ok(test_pattern(scene.width.max(2), scene.height.max(2)))),
        Source::Text(text) if !text.content.contains('{') => Some(Ok(render_text(text, &text.content))),
        _ => None,
    }
}

pub fn load(source: &Source, fps: u32, _size: (u32, u32)) -> Result<Loaded, String> {
    match source {
        Source::Image { path } => image::open(path)
            .map(|i| Loaded::Image(i.to_rgba8()))
            .map_err(|e| format!("couldn't open {}: {e}", path.display())),
        Source::Generated { generator, params, seed } => {
            let generator = crate::registry::find(generator).ok_or_else(|| format!("unknown generator {generator:?}"))?;
            (generator.generate)(params, *seed).map(Loaded::Image).map_err(|e| format!("{}: {e}", generator.name))
        }
        Source::Video { path, looping, speed } => VideoPlayer::start(path, fps, *looping, *speed)
            .map(|player| Loaded::Live(Box::new(player)))
            .map_err(|e| format!("{e:#}")),
        Source::Text(text) => Ok(Loaded::Live(Box::new(ClockText { text: text.clone(), last: String::new() }))),
        Source::Webcam { device } => super::webcam::Webcam::open(device).map(|w| Loaded::Live(Box::new(w))).map_err(|e| format!("{e:#}")),
        Source::Color { .. } | Source::TestPattern => Err("built on the render thread".into()),
    }
}

impl LiveSource for VideoPlayer {
    fn take_frame(&mut self) -> Option<RgbaImage> {
        VideoPlayer::take_frame(self)
    }

    fn error(&self) -> Option<String> {
        (self.has_ended() && self.frames_decoded() == 0).then(|| "the video produced no frames".to_string())
    }
}

/// Text with `{time}` / `{date}` placeholders, redrawn when the expanded text changes.
struct ClockText {
    text: TextSource,
    last: String,
}

impl LiveSource for ClockText {
    fn take_frame(&mut self) -> Option<RgbaImage> {
        let expanded = expand(&self.text.content);
        if expanded == self.last {
            return None;
        }
        self.last = expanded;
        Some(render_text(&self.text, &self.last))
    }
}

pub fn expand(content: &str) -> String {
    let now = chrono::Local::now();
    content
        .replace("{time}", &now.format("%H:%M:%S").to_string())
        .replace("{date}", &now.format("%d-%m-%Y").to_string())
        .replace("{datetime}", &now.format("%d-%m-%Y %H:%M:%S").to_string())
}

pub fn render_text(style: &TextSource, content: &str) -> RgbaImage {
    let font = if style.bold { &*BOLD } else { &*REGULAR };
    let size = style.size.clamp(6.0, 400.0);
    let lines: Vec<&str> = if content.is_empty() { vec![" "] } else { content.lines().collect() };
    let pad = (size * 0.25).ceil();
    let line_height = size * 1.3;
    let width = lines.iter().map(|l| text::width(font, size, l)).fold(1.0, f32::max) + pad * 2.0;
    let height = line_height * lines.len() as f32 + pad * 2.0;
    let mut canvas = Canvas::new(width.ceil() as u32, height.ceil() as u32, (0, 0, 0));
    canvas.pixmap.fill(tiny_skia::Color::TRANSPARENT);
    if let Some([r, g, b, a]) = style.background {
        canvas.fill_rect(0.0, 0.0, width, height, (r, g, b), a);
    }
    let [r, g, b, a] = style.color;
    let pen = Pen { font, size, color: (r, g, b), alpha: a };
    let shadow = Pen { color: (0, 0, 0), ..pen };
    let outline = (size / 16.0).max(1.0);
    for (i, line) in lines.iter().enumerate() {
        let at = (pad, pad + i as f32 * line_height);
        if style.outline {
            for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-0.7, -0.7), (0.7, -0.7), (-0.7, 0.7), (0.7, 0.7)] {
                text::draw(&mut canvas, shadow, (at.0 + dx * outline, at.1 + dy * outline), line, Anchor::LeftAscender);
            }
        }
        text::draw(&mut canvas, pen, at, line, Anchor::LeftAscender);
    }
    canvas.to_rgba()
}

/// Colour bars over a grey ramp with a centre cross, for checking framing and colour on the receiving end.
pub fn test_pattern(width: u32, height: u32) -> RgbaImage {
    const BARS: [[u8; 3]; 7] = [[192, 192, 192], [192, 192, 0], [0, 192, 192], [0, 192, 0], [192, 0, 192], [192, 0, 0], [0, 0, 192]];
    let bars_end = height * 2 / 3;
    let mut image = RgbaImage::from_fn(width, height, |x, y| {
        let [r, g, b] = if y < bars_end {
            BARS[(x * 7 / width).min(6) as usize]
        } else {
            let v = (x * 255 / width.max(1)) as u8;
            [v, v, v]
        };
        Rgba([r, g, b, 255])
    });
    let (cx, cy) = (width / 2, height / 2);
    for x in 0..width {
        image.put_pixel(x, cy, Rgba([255, 255, 255, 255]));
    }
    for y in 0..height {
        image.put_pixel(cx, y, Rgba([255, 255, 255, 255]));
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_grows_with_content_and_size() {
        let style = TextSource { outline: false, ..Default::default() };
        let short = render_text(&style, "A");
        let long = render_text(&style, "A much longer line");
        assert!(long.width() > short.width());
        let two = render_text(&style, "one\ntwo");
        assert!(two.height() > short.height());
    }

    #[test]
    fn placeholders_expand() {
        let expanded = expand("at {time} on {date}");
        assert!(!expanded.contains('{'), "{expanded}");
    }

    #[test]
    fn pattern_has_bars_and_a_cross() {
        let p = test_pattern(140, 90);
        assert_eq!(p.get_pixel(5, 5).0, [192, 192, 192, 255]);
        assert_eq!(p.get_pixel(135, 5).0, [0, 0, 192, 255]);
        assert_eq!(p.get_pixel(70, 10).0, [255, 255, 255, 255]);
    }
}
