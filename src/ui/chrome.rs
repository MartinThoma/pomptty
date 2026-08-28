//! The top bar: a minimal, Chrome-flavored tab strip.

use egui::{Button, Color32, RichText};

/// What the user asked the chrome to do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeAction {
    None,
    NewTab,
    SelectTab(usize),
    CloseTab(usize),
    /// The `Ctrl+R`-style search affordance was clicked. (Inert until the
    /// history milestone.)
    OpenSearch,
}

pub struct TabStrip<'a> {
    pub titles: &'a [String],
    pub active: usize,
    pub accent: Color32,
    pub muted: Color32,
}

impl TabStrip<'_> {
    pub fn show(self, ui: &mut egui::Ui) -> ChromeAction {
        let mut action = ChromeAction::None;

        egui::Panel::top("pomptty_tab_strip")
            .resizable(false)
            .show(ui, |ui| {
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    ui.add_space(4.0);

                    egui::ScrollArea::horizontal()
                        .max_width((ui.available_width() - 28.0).max(0.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (i, title) in self.titles.iter().enumerate() {
                                    self.tab(ui, i, title, &mut action);
                                    ui.add_space(2.0);
                                }
                                if ui
                                    .button("+")
                                    .on_hover_text("New tab  (Ctrl+Shift+T)")
                                    .clicked()
                                {
                                    action = ChromeAction::NewTab;
                                }
                            });
                        });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(4.0);
                        if ui
                            .button(RichText::new("⌕").color(self.muted))
                            .on_hover_text("Search history (Ctrl+R) — coming soon")
                            .clicked()
                        {
                            action = ChromeAction::OpenSearch;
                        }
                    });
                });
                ui.add_space(3.0);

                let rect = ui.max_rect();
                ui.painter().hline(
                    rect.left()..=rect.right(),
                    rect.bottom(),
                    egui::Stroke::new(1.0, self.accent.gamma_multiply(0.5)),
                );
            });

        action
    }

    fn tab(&self, ui: &mut egui::Ui, i: usize, title: &str, action: &mut ChromeAction) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;

            let pill = ui.selectable_label(i == self.active, truncate(title, 24));
            if pill.clicked() {
                *action = ChromeAction::SelectTab(i);
            }
            if pill.clicked_by(egui::PointerButton::Middle) {
                *action = ChromeAction::CloseTab(i);
            }
            pill.on_hover_text(title);

            let close =
                ui.add(Button::new(RichText::new("✕").small().color(self.muted)).frame(false));
            if close.on_hover_text("Close tab  (Ctrl+Shift+W)").clicked() {
                *action = ChromeAction::CloseTab(i);
            }
        });
    }
}

/// Truncate to `max` characters on a char boundary, appending `…`.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let keep: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{keep}…")
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate("bash", 24), "bash");
    }

    #[test]
    fn truncate_shortens_long_strings() {
        let out = truncate("a-very-long-tab-title-that-keeps-going", 10);
        assert_eq!(out.chars().count(), 10);
        assert!(out.ends_with('…'));
    }
}
