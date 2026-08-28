//! The top bar. Deliberately minimal for milestone 1 — it becomes the tab strip
//! once multi-tab lands.

use egui::{Color32, RichText};

/// What the user asked the chrome to do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeAction {
    None,
    /// The `Ctrl+R`-style search affordance was clicked. (Inert until the
    /// history milestone.)
    OpenSearch,
}

pub struct TopBar<'a> {
    pub title: &'a str,
    pub accent: Color32,
    pub muted: Color32,
    /// Shown right-aligned; a short status string or the current directory.
    pub subtitle: Option<&'a str>,
}

impl TopBar<'_> {
    pub fn show(self, ui: &mut egui::Ui) -> ChromeAction {
        let mut action = ChromeAction::None;

        egui::Panel::top("pomptty_top_bar")
            .resizable(false)
            .show(ui, |ui| {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new(self.title).strong());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(4.0);
                        if ui
                            .button(RichText::new("⌕").color(self.muted))
                            .on_hover_text("Search history (Ctrl+R) — coming soon")
                            .clicked()
                        {
                            action = ChromeAction::OpenSearch;
                        }
                        if let Some(sub) = self.subtitle {
                            ui.label(RichText::new(sub).color(self.muted).small());
                        }
                    });
                });
                ui.add_space(2.0);

                // Accent hairline, Chrome-style.
                let rect = ui.max_rect();
                let y = rect.bottom();
                ui.painter().hline(
                    rect.left()..=rect.right(),
                    y,
                    egui::Stroke::new(1.0, self.accent.gamma_multiply(0.5)),
                );
            });

        action
    }
}
