//! Phosphor Regular glyphs, embedded as a font so they work in any label, button or hint text.
//! Codepoints come from @phosphor-icons/web 2.1.1 `src/regular/style.css`.
#![allow(dead_code)] // the set we draw from, not a list of glyphs in use

pub const ARROWS_OUT: &str = "\u{E0A2}";
pub const ARROW_CLOCKWISE: &str = "\u{E036}";
pub const ARROW_DOWN: &str = "\u{E03E}";
pub const ARROW_SQUARE_OUT: &str = "\u{E5DE}";
pub const ARROW_UP: &str = "\u{E08E}";
pub const ARROW_U_UP_LEFT: &str = "\u{E08A}";
pub const ARROW_U_UP_RIGHT: &str = "\u{E08C}";
pub const BANK: &str = "\u{E0B4}";
pub const BARCODE: &str = "\u{E0B8}";
pub const BROADCAST: &str = "\u{E0F2}";
pub const CAMERA: &str = "\u{E10E}";
pub const CARET_DOWN: &str = "\u{E136}";
pub const CHECK: &str = "\u{E182}";
pub const CLOCK: &str = "\u{E19A}";
pub const COPY: &str = "\u{E1CA}";
pub const CREDIT_CARD: &str = "\u{E1D2}";
pub const CROP: &str = "\u{E1D4}";
pub const CURSOR: &str = "\u{E1DC}";
pub const DICE_FIVE: &str = "\u{E1EE}";
pub const DOWNLOAD_SIMPLE: &str = "\u{E20C}";
pub const EYE: &str = "\u{E220}";
pub const EYE_SLASH: &str = "\u{E224}";
pub const FILE: &str = "\u{E230}";
pub const FILE_X: &str = "\u{E23C}";
pub const FILM_STRIP: &str = "\u{E792}";
pub const FINGERPRINT: &str = "\u{E23E}";
pub const FLASK: &str = "\u{E79E}";
pub const FLOPPY_DISK: &str = "\u{E248}";
pub const FOLDER: &str = "\u{E24A}";
pub const FOLDER_OPEN: &str = "\u{E256}";
pub const GEAR: &str = "\u{E270}";
pub const HOUSE: &str = "\u{E2C2}";
pub const IDENTIFICATION_CARD: &str = "\u{E2C8}";
pub const IMAGE: &str = "\u{E2CA}";
pub const IMAGE_BROKEN: &str = "\u{E7A8}";
pub const LOCK: &str = "\u{E2FA}";
pub const LOCK_OPEN: &str = "\u{E306}";
pub const MAGNIFYING_GLASS: &str = "\u{E30C}";
pub const MOON: &str = "\u{E330}";
pub const PALETTE: &str = "\u{E6C8}";
pub const PAUSE: &str = "\u{E39E}";
pub const PLAY: &str = "\u{E3D0}";
pub const PLUS: &str = "\u{E3D4}";
pub const QR_CODE: &str = "\u{E3E6}";
pub const RECORD: &str = "\u{E3EE}";
pub const SCAN: &str = "\u{EBB6}";
pub const SHUFFLE: &str = "\u{E422}";
pub const SQUARES_FOUR: &str = "\u{E464}";
pub const STACK: &str = "\u{E466}";
pub const STOP: &str = "\u{E46C}";
pub const SUN: &str = "\u{E472}";
pub const TABLE: &str = "\u{E476}";
pub const TEXT_T: &str = "\u{E48A}";
pub const TRASH: &str = "\u{E4A6}";
pub const USERS: &str = "\u{E4D6}";
pub const USER_LIST: &str = "\u{E73C}";
pub const VIDEO_CAMERA: &str = "\u{E4DA}";
pub const WARNING: &str = "\u{E4E0}";
pub const WEBCAM: &str = "\u{E9B2}";
pub const X: &str = "\u{E4F6}";

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    fn is_emoji(c: char) -> bool {
        matches!(c as u32, 0x1F300..=0x1FAFF | 0x2600..=0x27BF)
    }

    #[test]
    fn ui_strings_use_icons_not_emoji() {
        let mut files = Vec::new();
        rust_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let offenders: Vec<String> = files
            .iter()
            .flat_map(|file| {
                let text = std::fs::read_to_string(file).unwrap();
                text.lines()
                    .enumerate()
                    .filter(|(_, line)| !line.trim_start().starts_with("//") && line.chars().any(is_emoji))
                    .map(|(n, line)| format!("{}:{}: {}", file.display(), n + 1, line.trim()))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert!(offenders.is_empty(), "use ui::icons instead of emoji:\n{}", offenders.join("\n"));
    }
}
