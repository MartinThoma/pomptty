//! Font setup.
//!
//! Without `font_family` set, the bundled monospace face is used. `font_family`
//! may be either a path to a `.ttf`/`.otf`/`.ttc` file or the name of an
//! installed font family (looked up via `fontdb` / the system font
//! directories); if it resolves to nothing, the bundled font is kept.
//!
//! Regardless of `font_family`, an installed Nerd / Powerline font (if any) is
//! appended to the fallback chain so powerline prompts and devicon themes render
//! their private-use glyphs instead of boxes. Nothing in egui's default chain
//! covers that range.

use std::sync::{Arc, OnceLock};

use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use fontdb::{Database, FaceInfo, Family, Query, Style};

use crate::config::Config;

const USER_FONT: &str = "pomptty-user-font";
const SYMBOL_FONT: &str = "pomptty-symbols";

/// The system font database, scanned once per process (config reloads reuse it).
fn font_db() -> &'static Database {
    static DB: OnceLock<Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = Database::new();
        db.load_system_fonts();
        db
    })
}

/// Apply the configured font to the egui context. Safe to call again on reload.
pub fn apply(ctx: &egui::Context, config: &Config) {
    let mut fonts = FontDefinitions::default();

    if let Some(spec) = &config.font_family {
        match resolve(spec) {
            Some(font) => {
                fonts.font_data.insert(
                    USER_FONT.to_owned(),
                    Arc::new(FontData {
                        font: font.bytes.into(),
                        index: font.index,
                        tweak: FontTweak::default(),
                    }),
                );
                for family in [FontFamily::Monospace, FontFamily::Proportional] {
                    fonts
                        .families
                        .entry(family)
                        .or_default()
                        .insert(0, USER_FONT.to_owned());
                }
                log::info!("using font {spec:?} ({})", font.origin);
            }
            None => log::warn!(
                "font_family {spec:?}: not a readable font file and no installed \
                 family by that name — keeping the bundled font"
            ),
        }
    }

    match find_symbol_font(font_db()) {
        Some((name, bytes, index)) => {
            fonts.font_data.insert(
                SYMBOL_FONT.to_owned(),
                Arc::new(FontData {
                    font: bytes.into(),
                    index,
                    tweak: FontTweak::default(),
                }),
            );
            // Append: real glyphs and emoji still win; only the private-use
            // icon range falls through to here.
            for family in [FontFamily::Monospace, FontFamily::Proportional] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push(SYMBOL_FONT.to_owned());
            }
            log::info!("glyph fallback: {name:?}");
        }
        None => log::debug!("no Nerd/Powerline font found; icon glyphs may render as boxes"),
    }

    ctx.set_fonts(fonts);
}

/// Pick an installed Nerd / Powerline font for the private-use icon range,
/// preferring a Nerd Font, then a `Mono` variant, then a regular weight.
fn find_symbol_font(db: &Database) -> Option<(String, Vec<u8>, u32)> {
    let id = ["nerd font", "powerline"].iter().find_map(|keyword| {
        db.faces()
            .filter(|f| {
                f.families
                    .iter()
                    .any(|(fam, _)| fam.to_lowercase().contains(keyword))
            })
            .min_by_key(|f| {
                let not_mono = u8::from(
                    !f.families
                        .iter()
                        .any(|(fam, _)| fam.to_lowercase().contains("mono")),
                );
                (not_mono, face_rank(f))
            })
            .map(|f| f.id)
    })?;

    let name = db
        .face(id)
        .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
        .unwrap_or_default();
    let (bytes, index) = db.with_face_data(id, |data, index| (data.to_vec(), index))?;
    Some((name, bytes, index))
}

struct ResolvedFont {
    bytes: Vec<u8>,
    /// Face index within the file (nonzero only for `.ttc` collections).
    index: u32,
    /// Where the bytes came from, for the log line.
    origin: String,
}

/// Resolve `spec` — a path to a font file, or an installed family name — to
/// concrete font bytes.
fn resolve(spec: &str) -> Option<ResolvedFont> {
    let path = std::path::Path::new(spec);
    if path.is_file() {
        return match std::fs::read(path) {
            Ok(bytes) => Some(ResolvedFont {
                bytes,
                index: 0,
                origin: format!("file {}", path.display()),
            }),
            Err(e) => {
                log::warn!("could not read font file {}: {e}", path.display());
                None
            }
        };
    }
    resolve_family(spec)
}

fn resolve_family(name: &str) -> Option<ResolvedFont> {
    let db = font_db();

    let id = db
        .query(&Query {
            families: &[Family::Name(name)],
            ..Query::default()
        })
        .or_else(|| best_face_ci(db, name))?;

    let post_script = db
        .face(id)
        .map(|f| f.post_script_name.clone())
        .unwrap_or_default();
    let (bytes, index) = db.with_face_data(id, |data, index| (data.to_vec(), index))?;
    Some(ResolvedFont {
        bytes,
        index,
        origin: format!("installed family {name:?} -> {post_script}"),
    })
}

/// Case-insensitive family match — `fontdb`'s own `query` compares names exactly
/// — preferring the face nearest a regular weight and upright style.
fn best_face_ci(db: &'static Database, name: &str) -> Option<fontdb::ID> {
    let want = name.to_lowercase();
    db.faces()
        .filter(|f| {
            f.families
                .iter()
                .any(|(family, _)| family.to_lowercase() == want)
        })
        .min_by_key(|f| face_rank(f))
        .map(|f| f.id)
}

fn face_rank(f: &FaceInfo) -> (u16, u8) {
    let weight_dist = f.weight.0.abs_diff(400);
    let style_rank = match f.style {
        Style::Normal => 0,
        Style::Oblique => 1,
        Style::Italic => 2,
    };
    (weight_dist, style_rank)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bogus_input() {
        assert!(resolve("/no/such/font/file.ttf").is_none());
        assert!(resolve("Definitely Not An Installed Family 9000").is_none());
        assert!(resolve("").is_none());
    }

    #[test]
    fn resolves_an_installed_family_by_name() {
        let mut db = Database::new();
        db.load_system_fonts();
        let Some(face) = db.faces().find(|f| !f.families.is_empty()) else {
            return; // no system fonts available (unusual) — nothing to check
        };
        let family = face.families[0].0.clone();

        let by_exact = resolve(&family).expect("exact family name should resolve");
        assert!(!by_exact.bytes.is_empty());

        // Case folding: our fallback path should still find it.
        let weird_case = family.to_uppercase();
        if weird_case != family {
            assert!(
                resolve(&weird_case).is_some(),
                "case-insensitive lookup failed for {weird_case:?}"
            );
        }
    }
}
