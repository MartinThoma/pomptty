use egui::{Context, FontId};

use crate::types::Size;

#[derive(Debug, Clone)]
pub struct FontSettings {
    pub font_type: FontId,
    /// Real bold/italic/bold-italic faces, if the caller resolved any —
    /// each defaults to `font_type` (so a caller that only sets `font_type`
    /// keeps behaving exactly as before this field existed).
    pub bold: Option<FontId>,
    pub italic: Option<FontId>,
    pub bold_italic: Option<FontId>,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            font_type: FontId::monospace(14.0),
            bold: None,
            italic: None,
            bold_italic: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerminalFont {
    font_type: FontId,
    bold: FontId,
    italic: FontId,
    bold_italic: FontId,
}

impl Default for TerminalFont {
    fn default() -> Self {
        Self::new(FontSettings::default())
    }
}

impl TerminalFont {
    pub fn new(settings: FontSettings) -> Self {
        let regular = settings.font_type;
        Self {
            bold: settings.bold.unwrap_or_else(|| regular.clone()),
            italic: settings.italic.unwrap_or_else(|| regular.clone()),
            bold_italic: settings.bold_italic.unwrap_or_else(|| regular.clone()),
            font_type: regular,
        }
    }

    pub fn font_type(&self) -> FontId {
        self.font_type.clone()
    }

    /// The face to use for a cell with the given `bold`/`italic` flags —
    /// whichever of the four faces matches, falling back to the regular one
    /// for any style the caller didn't provide a real face for.
    pub fn font_id(&self, bold: bool, italic: bool) -> FontId {
        match (bold, italic) {
            (true, true) => self.bold_italic.clone(),
            (true, false) => self.bold.clone(),
            (false, true) => self.italic.clone(),
            (false, false) => self.font_type.clone(),
        }
    }

    pub fn font_measure(&self, ctx: &Context) -> Size {
        let (width, height) = ctx.fonts_mut(|f| {
            (
                f.glyph_width(&self.font_type, 'm'),
                f.row_height(&self.font_type),
            )
        });

        Size::new(width, height)
    }
}
