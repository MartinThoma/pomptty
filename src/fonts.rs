//! Font setup.
//!
//! Milestone 1: the bundled monospace face is used unless `font_family` points
//! at a font file on disk. Looking up an installed font by family name comes in
//! a later milestone.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

use crate::config::Config;

const USER_FONT: &str = "pomptty-user-font";

/// Apply the configured font to the egui context. Safe to call again on reload.
pub fn apply(ctx: &egui::Context, config: &Config) {
    let mut fonts = FontDefinitions::default();

    if let Some(family) = &config.font_family {
        let path = std::path::Path::new(family);
        if path.is_file() {
            match std::fs::read(path) {
                Ok(bytes) => {
                    fonts
                        .font_data
                        .insert(USER_FONT.to_owned(), Arc::new(FontData::from_owned(bytes)));
                    fonts
                        .families
                        .entry(FontFamily::Monospace)
                        .or_default()
                        .insert(0, USER_FONT.to_owned());
                    fonts
                        .families
                        .entry(FontFamily::Proportional)
                        .or_default()
                        .insert(0, USER_FONT.to_owned());
                    log::info!("loaded font from {}", path.display());
                }
                Err(e) => log::warn!("could not read font file {}: {e}", path.display()),
            }
        } else {
            log::warn!(
                "font_family {family:?} is not a font file on disk; \
                 system-font lookup is not implemented yet, using the bundled font"
            );
        }
    }

    ctx.set_fonts(fonts);
}
