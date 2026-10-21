//! Ease of access: the corner button and flyout (modelled on the Windows 7–10 sign-in screen's
//! accessibility menu), plus ProjectIle-specific helpers for first-time users.

use crate::theme::Palette;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Access {
    pub magnifier: bool,
    pub high_contrast: bool,
    pub large_text: bool,
    pub big_targets: bool,
    pub reduce_motion: bool,
    pub strong_focus: bool,
    /// Show a plain-language explanation under every field and option.
    pub explain: bool,
}

impl Access {
    /// Every toggle with its label and one-line description, in flyout order.
    pub fn toggles(&mut self) -> [(&'static str, &'static str, &mut bool); 7] {
        [
            ("Magnifier", "Enlarge everything by 25%", &mut self.magnifier),
            ("High contrast", "Black, white and yellow, thicker borders", &mut self.high_contrast),
            ("Larger text", "Text 20% bigger, layout unchanged", &mut self.large_text),
            ("Bigger click targets", "Taller buttons, switches and rows", &mut self.big_targets),
            ("Reduce motion", "No scrolling or fade animations", &mut self.reduce_motion),
            ("Strong focus outline", "See which control has keyboard focus", &mut self.strong_focus),
            ("Beginner explanations", "Explain every field in plain words", &mut self.explain),
        ]
    }

    pub fn active_count(&self) -> usize {
        [self.magnifier, self.high_contrast, self.large_text, self.big_targets, self.reduce_motion, self.strong_focus, self.explain]
            .iter()
            .filter(|on| **on)
            .count()
    }
}

/// An On/Off switch like the Windows 10 accessibility flyout's.
pub fn switch(ui: &mut egui::Ui, on: &mut bool, label: &str, palette: &Palette) -> egui::Response {
    let size = egui::vec2(ui.spacing().interact_size.y * 2.0, ui.spacing().interact_size.y);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, label));
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let radius = rect.height() / 2.0;
        let fill = if *on { palette.accent } else { palette.field };
        let stroke = if response.has_focus() { Stroke::new(2.0, palette.accent) } else { Stroke::new(1.0, palette.text_dim) };
        ui.painter().rect(rect.shrink(1.0), radius, fill, stroke, egui::StrokeKind::Inside);
        let knob_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        let knob = if *on { palette.accent_text } else { palette.text };
        ui.painter().circle_filled(Pos2::new(knob_x, rect.center().y), radius * 0.62, knob);
    }
    response
}

/// Draws an original ease-of-access glyph: a broken ring with an arrowhead around a dot.
pub fn paint_icon(painter: &egui::Painter, rect: Rect, color: Color32) {
    let center = rect.center();
    let r = rect.width().min(rect.height()) * 0.36;
    let stroke = Stroke::new(1.8, color);
    let steps = 28;
    let points: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + 0.5 + (i as f32 / steps as f32) * (std::f32::consts::TAU - 1.0);
            center + Vec2::angled(a) * r
        })
        .collect();
    for pair in points.chunks(2) {
        if let [a, b] = pair {
            painter.line_segment([*a, *b], stroke);
        }
    }
    if let Some(end) = points.last() {
        let tangent = Vec2::angled(-std::f32::consts::FRAC_PI_2 + 0.5 + std::f32::consts::TAU - 1.0 + std::f32::consts::FRAC_PI_2);
        let normal = tangent.rot90();
        painter.line_segment([*end, *end - tangent * 4.0 + normal * 4.0], stroke);
        painter.line_segment([*end, *end - tangent * 4.0 - normal * 4.0], stroke);
    }
    painter.circle_filled(center, r * 0.28, color);
}

/// The square corner button that opens the flyout.
pub fn button(ui: &mut egui::Ui, palette: &Palette, open: bool, active: usize) -> egui::Response {
    let side = ui.spacing().interact_size.y + 6.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Ease of access"));
    let fill = if open || response.hovered() { palette.hover } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, f32::from(palette.radius), fill);
    if response.has_focus() {
        ui.painter().rect_stroke(rect, f32::from(palette.radius), Stroke::new(2.0, palette.accent), egui::StrokeKind::Inside);
    }
    paint_icon(ui.painter(), rect, if active > 0 { palette.accent } else { palette.text });
    response.on_hover_text("Ease of access (Ctrl+Alt+U)")
}

/// The flyout content. Returns `true` when a setting changed (re-apply the theme).
pub fn flyout(ui: &mut egui::Ui, access: &mut Access, palette: &Palette) -> bool {
    let mut changed = false;
    ui.set_width(300.0);
    ui.label(egui::RichText::new("Ease of access").heading().color(palette.text));
    ui.label(egui::RichText::new("Applies to ProjectIle only. Saved between launches.").small().color(palette.text_dim));
    ui.add_space(6.0);
    for (label, description, value) in access.toggles() {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(label).strong());
                ui.label(egui::RichText::new(if *value { "On" } else { "Off" }).small().strong().color(if *value { palette.accent } else { palette.text_dim }));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if switch(ui, value, label, palette).on_hover_text(description).changed() {
                    changed = true;
                }
            });
        });
        ui.add_space(2.0);
    }
    ui.separator();
    ui.label(egui::RichText::new("Screen readers").strong());
    ui.label(
        egui::RichText::new("ProjectIle exposes its controls through AccessKit, so Orca, Narrator and VoiceOver can read them.")
            .small()
            .color(palette.text_dim),
    );
    ui.label(egui::RichText::new("Keyboard").strong());
    ui.label(
        egui::RichText::new("Tab / Shift+Tab move between controls · Space toggles · Ctrl+1–5 switch tabs · Ctrl+G generates · Ctrl+= / Ctrl+− zoom · F1 help")
            .small()
            .color(palette.text_dim),
    );
    ui.add_space(4.0);
    if ui.button("Turn everything off").clicked() {
        *access = Access::default();
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::Access;

    #[test]
    fn toggles_cover_every_setting() {
        let mut access = Access::default();
        for (_, _, value) in access.toggles() {
            *value = true;
        }
        assert_eq!(access.active_count(), 7);
        let restored: Access = serde_json::from_str("{}").unwrap_or_default();
        assert_eq!(restored, Access::default());
    }
}
