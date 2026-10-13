//! Upload edge cases: files that are too big, lie about their type, are broken, or have awkward names.

use std::io::Write;
use anyhow::Result;
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use crate::render::Canvas;
use crate::render::text::{self, Anchor, BOLD, Pen};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Base {
    Png,
    Jpeg,
    Pdf,
    Zip,
    Text,
}

impl Base {
    pub const ALL: [Base; 5] = [Base::Png, Base::Jpeg, Base::Pdf, Base::Zip, Base::Text];

    pub fn ext(self) -> &'static str {
        match self {
            Base::Png => "png",
            Base::Jpeg => "jpg",
            Base::Pdf => "pdf",
            Base::Zip => "zip",
            Base::Text => "txt",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Case {
    /// A valid file padded to `bytes` (after the image end marker, or in a PDF comment).
    Sized { base: Base, bytes: u64 },
    /// `base` content saved under another extension.
    WrongExtension { base: Base, ext: String },
    /// The magic bytes at the start are scrambled.
    CorruptHeader { base: Base },
    /// Only the first `percent` of the file.
    Truncated { base: Base, percent: u8 },
    Empty { ext: String },
    /// A tiny PNG file that claims (and decodes to) huge dimensions: a decompression bomb.
    HugeImage { width: u32, height: u32 },
    /// A JPEG whose EXIF orientation tag says to rotate it; the drawn arrow shows which way is up.
    ExifRotated { orientation: u16 },
    /// A valid PNG with a name that tends to break things.
    AwkwardName { name: NameStyle },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NameStyle {
    /// 255 bytes, the usual filesystem limit.
    Long,
    Unicode,
    DoubleExtension,
    Spaces,
    UpperCaseExtension,
    NoExtension,
}

impl NameStyle {
    pub const ALL: [NameStyle; 6] =
        [NameStyle::Long, NameStyle::Unicode, NameStyle::DoubleExtension, NameStyle::Spaces, NameStyle::UpperCaseExtension, NameStyle::NoExtension];

    pub fn label(self) -> &'static str {
        match self {
            NameStyle::Long => "255-byte name",
            NameStyle::Unicode => "Unicode and emoji",
            NameStyle::DoubleExtension => "Double extension (.jpg.exe)",
            NameStyle::Spaces => "Spaces and brackets",
            NameStyle::UpperCaseExtension => "Upper-case extension",
            NameStyle::NoExtension => "No extension",
        }
    }

    fn file_name(self) -> String {
        match self {
            NameStyle::Long => format!("{}.png", "a".repeat(251)),
            NameStyle::Unicode => "ktp_kucing_ñ_ü_日本_🐱.png".into(),
            NameStyle::DoubleExtension => "photo.jpg.exe".into(),
            NameStyle::Spaces => "my photo (1) [final].png".into(),
            NameStyle::UpperCaseExtension => "SELFIE.PNG".into(),
            NameStyle::NoExtension => "selfie".into(),
        }
    }
}

pub struct TestFile {
    pub name: String,
    pub bytes: Vec<u8>,
    /// What the file is for, shown next to it.
    pub note: String,
}

fn sample_image(width: u32, height: u32, label: &str) -> RgbaImage {
    let mut canvas = Canvas::new(width, height, (40, 110, 200));
    let (w, h) = (width as f32, height as f32);
    canvas.fill_polygon(&[(w / 2.0, h * 0.12), (w * 0.75, h * 0.45), (w * 0.25, h * 0.45)], (255, 255, 255));
    canvas.fill_rect(w * 0.43, h * 0.45, w * 0.57, h * 0.7, (255, 255, 255), 255);
    text::draw(&mut canvas, Pen::new(&BOLD, h * 0.14, (255, 255, 255)), (w / 2.0, h * 0.76), label, Anchor::MiddleTop);
    canvas.to_rgba()
}

fn encode(image: &RgbaImage, format: image::ImageFormat) -> Result<Vec<u8>> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    let dynamic = image::DynamicImage::ImageRgba8(image.clone());
    let dynamic = if format == image::ImageFormat::Jpeg { image::DynamicImage::ImageRgb8(dynamic.to_rgb8()) } else { dynamic };
    dynamic.write_to(&mut bytes, format)?;
    Ok(bytes.into_inner())
}

/// One page with a line of text, cross-reference offsets computed so strict readers accept it.
fn minimal_pdf(padding: usize) -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        {
            let stream = "BT /F1 24 Tf 72 760 Td (Testing Tools sample PDF) Tj ET";
            format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len())
        },
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    if padding > 0 {
        let line = "% padding padding padding padding padding padding padding padding padding\n";
        let mut left = padding;
        while left > 0 {
            let take = left.min(line.len());
            out.extend_from_slice(&line.as_bytes()[..take]);
            left -= take;
        }
        if !out.ends_with(b"\n") {
            out.push(b'\n');
        }
    }
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}

/// A zip with one stored (uncompressed) text file.
fn minimal_zip() -> Vec<u8> {
    let name = b"readme.txt";
    let data = b"Testing Tools sample archive\n";
    let crc = crc32fast::hash(data);
    let mut out = Vec::new();
    let local = [&0x0403_4b50u32.to_le_bytes()[..], &20u16.to_le_bytes(), &0u16.to_le_bytes(), &0u16.to_le_bytes(), &0u32.to_le_bytes()].concat();
    out.extend_from_slice(&local);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(name);
    out.extend_from_slice(data);
    let central_at = out.len();
    out.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
    out.extend_from_slice(&20u16.to_le_bytes());
    out.extend_from_slice(&20u16.to_le_bytes());
    out.extend_from_slice(&[0; 2 + 2 + 4]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
    out.extend_from_slice(&[0; 2 + 2 + 2 + 2 + 4]);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(name);
    let central_len = out.len() - central_at;
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(central_len as u32).to_le_bytes());
    out.extend_from_slice(&(central_at as u32).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

pub fn base_bytes(base: Base) -> Result<Vec<u8>> {
    Ok(match base {
        Base::Png => encode(&sample_image(256, 256, "PNG"), image::ImageFormat::Png)?,
        Base::Jpeg => encode(&sample_image(256, 256, "JPEG"), image::ImageFormat::Jpeg)?,
        Base::Pdf => minimal_pdf(0),
        Base::Zip => minimal_zip(),
        Base::Text => b"Testing Tools sample text file\n".to_vec(),
    })
}

fn padded(base: Base, bytes: u64) -> Result<Vec<u8>> {
    let target = bytes as usize;
    let mut out = match base {
        Base::Pdf => {
            // The startxref number's width shifts with the size, so settle on the padding in a few passes.
            let mut padding = target.saturating_sub(minimal_pdf(0).len());
            let mut pdf = minimal_pdf(padding);
            for _ in 0..4 {
                if pdf.len() == target || padding == 0 {
                    break;
                }
                padding = (padding as i64 + target as i64 - pdf.len() as i64).max(0) as usize;
                pdf = minimal_pdf(padding);
            }
            pdf
        }
        _ => base_bytes(base)?,
    };
    // Image and archive readers stop at their end marker, so trailing bytes keep the file valid.
    if out.len() < target {
        out.resize(target, 0);
    }
    Ok(out)
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(kind);
    hasher.update(data);
    out.extend_from_slice(&hasher.finalize().to_be_bytes());
}

/// Streams zero rows through deflate, so even a 50k×50k image costs a few KB and no big buffer.
fn huge_png(width: u32, height: u32) -> Result<Vec<u8>> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 0, 0, 0, 0]);
    png_chunk(&mut out, b"IHDR", &header);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let row = vec![0u8; width as usize + 1];
    for _ in 0..height {
        encoder.write_all(&row)?;
    }
    png_chunk(&mut out, b"IDAT", &encoder.finish()?);
    png_chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

/// Inserts an APP1 Exif segment holding only the orientation tag right after the JPEG's SOI marker.
fn with_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    let mut tiff = b"II*\0".to_vec();
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&1u16.to_le_bytes());
    tiff.extend_from_slice(&0x0112u16.to_le_bytes());
    tiff.extend_from_slice(&3u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&[0, 0]);
    tiff.extend_from_slice(&0u32.to_le_bytes());
    let mut segment = b"Exif\0\0".to_vec();
    segment.extend_from_slice(&tiff);
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((segment.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&segment);
    out.extend_from_slice(&jpeg[2..]);
    out
}

pub fn make(case: &Case) -> Result<TestFile> {
    Ok(match case {
        Case::Sized { base, bytes } => TestFile {
            name: format!("sized-{}.{}", human_size(*bytes).replace(' ', ""), base.ext()),
            bytes: padded(*base, *bytes)?,
            note: format!("a valid {} file of {}", base.ext().to_uppercase(), human_size(*bytes)),
        },
        Case::WrongExtension { base, ext } => TestFile {
            name: format!("really-{}.{}", base.ext(), ext.trim_start_matches('.')),
            bytes: base_bytes(*base)?,
            note: format!("{} content named .{ext}", base.ext().to_uppercase()),
        },
        Case::CorruptHeader { base } => {
            let mut bytes = base_bytes(*base)?;
            for b in bytes.iter_mut().take(8) {
                *b ^= 0x5A;
            }
            TestFile { name: format!("corrupt-header.{}", base.ext()), bytes, note: "first 8 bytes scrambled".into() }
        }
        Case::Truncated { base, percent } => {
            let mut bytes = base_bytes(*base)?;
            bytes.truncate(bytes.len() * usize::from((*percent).clamp(1, 99)) / 100);
            TestFile { name: format!("truncated-{percent}pct.{}", base.ext()), bytes, note: format!("only the first {percent}% of the file") }
        }
        Case::Empty { ext } => TestFile { name: format!("empty.{}", ext.trim_start_matches('.')), bytes: Vec::new(), note: "zero bytes".into() },
        Case::HugeImage { width, height } => {
            let bytes = huge_png(*width, *height)?;
            TestFile {
                name: format!("huge-{width}x{height}.png"),
                note: format!("{} on disk, {} once decoded", human_size(bytes.len() as u64), human_size(u64::from(*width) * u64::from(*height))),
                bytes,
            }
        }
        Case::ExifRotated { orientation } => {
            let jpeg = encode(&sample_image(400, 300, "TOP"), image::ImageFormat::Jpeg)?;
            TestFile {
                name: format!("exif-orientation-{orientation}.jpg"),
                bytes: with_orientation(&jpeg, *orientation),
                note: format!("EXIF orientation {orientation}: apps that honour it show the arrow pointing up"),
            }
        }
        Case::AwkwardName { name } => TestFile { name: name.file_name(), bytes: base_bytes(Base::Png)?, note: name.label().into() },
    })
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", UNITS[unit]).replace(".0 ", " ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sized_files_hit_the_size_and_stay_valid() {
        for base in [Base::Png, Base::Jpeg, Base::Pdf] {
            let file = make(&Case::Sized { base, bytes: 3 * 1024 * 1024 }).unwrap();
            assert_eq!(file.bytes.len(), 3 * 1024 * 1024, "{base:?}");
            if base != Base::Pdf {
                assert!(image::load_from_memory(&file.bytes).is_ok(), "{base:?}");
            } else {
                assert!(file.bytes.starts_with(b"%PDF-1.4") && file.bytes.ends_with(b"%%EOF\n"));
            }
        }
    }

    #[test]
    fn pdf_xref_offsets_point_at_objects() {
        let pdf = minimal_pdf(100);
        let text = String::from_utf8(pdf.clone()).unwrap();
        let xref_at: usize = text.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(text[xref_at..].starts_with("xref"));
        let first: usize = text[xref_at..].lines().nth(3).unwrap()[..10].parse().unwrap();
        assert!(text[first..].starts_with("1 0 obj"));
    }

    #[test]
    fn broken_files_are_broken() {
        let corrupt = make(&Case::CorruptHeader { base: Base::Png }).unwrap();
        assert!(image::load_from_memory(&corrupt.bytes).is_err());
        let cut = make(&Case::Truncated { base: Base::Png, percent: 50 }).unwrap();
        assert!(image::load_from_memory(&cut.bytes).is_err());
        assert!(make(&Case::Empty { ext: "jpg".into() }).unwrap().bytes.is_empty());
    }

    #[test]
    fn huge_png_is_small_on_disk_but_claims_its_size() {
        let file = make(&Case::HugeImage { width: 20_000, height: 20_000 }).unwrap();
        assert!(file.bytes.len() < 1024 * 1024, "{} bytes", file.bytes.len());
        let reader = image::ImageReader::new(std::io::Cursor::new(&file.bytes)).with_guessed_format().unwrap();
        assert_eq!(reader.into_dimensions().unwrap(), (20_000, 20_000));
    }

    #[test]
    fn exif_rotated_jpeg_still_decodes_and_carries_the_tag() {
        let file = make(&Case::ExifRotated { orientation: 6 }).unwrap();
        assert!(image::load_from_memory(&file.bytes).is_ok());
        assert!(file.bytes.windows(6).any(|w| w == b"Exif\0\0"));
    }

    #[test]
    fn zip_and_names() {
        let zip = minimal_zip();
        assert!(zip.starts_with(b"PK\x03\x04") && zip.windows(4).any(|w| w == b"PK\x05\x06"));
        assert_eq!(NameStyle::Long.file_name().len(), 255);
        assert_eq!(human_size(3 * 1024 * 1024), "3 MB");
        assert_eq!(human_size(1536), "1.5 KB");
    }
}
