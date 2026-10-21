//! ProjectIle's look: PhotoCraft's own palettes (the Photoshop-style Pro themes and the violet
//! Studio themes from `crates/ui-egui/src/theme.rs`), plus the ease-of-access adjustments.

use crate::access::Access;
use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppTheme {
    /// PhotoCraft Pro: Photoshop-style charcoal with a blue accent.
    #[default]
    #[serde(alias = "Dark")]
    Pro,
    /// PhotoCraft Pro Medium: Photoshop's default #535353 brightness.
    ProMedium,
    /// PhotoCraft Studio: near-black with the soft violet accent.
    Studio,
    #[serde(alias = "Light")]
    StudioLight,
}

impl AppTheme {
    pub const ALL: [AppTheme; 4] = [AppTheme::Pro, AppTheme::ProMedium, AppTheme::Studio, AppTheme::StudioLight];

    pub fn label(self) -> &'static str {
        match self {
            Self::Pro => "Pro Dark",
            Self::ProMedium => "Pro Medium",
            Self::Studio => "Studio",
            Self::StudioLight => "Studio Light",
        }
    }
}

/// Colours every widget reads; nothing in the UI hard-codes a colour.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub chrome: Color32,
    pub background: Color32,
    pub card: Color32,
    pub card_border: Color32,
    pub field: Color32,
    /// Outline of inputs, checkboxes and buttons (PhotoCraft's `field_border` token).
    pub field_border: Color32,
    pub hover: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub added: Color32,
    pub removed: Color32,
    pub changed: Color32,
    pub warning: Color32,
    pub tab_strip: Color32,
    pub radius: u8,
}

impl Palette {
    pub fn get(theme: AppTheme, access: &Access) -> Self {
        if access.high_contrast {
            return Self {
                dark: true,
                chrome: Color32::BLACK,
                background: Color32::BLACK,
                card: Color32::BLACK,
                card_border: Color32::WHITE,
                field: Color32::BLACK,
                field_border: Color32::WHITE,
                hover: Color32::from_rgb(40, 40, 40),
                text: Color32::WHITE,
                text_dim: Color32::from_rgb(230, 230, 230),
                accent: Color32::from_rgb(255, 230, 0),
                accent_text: Color32::BLACK,
                added: Color32::from_rgb(80, 255, 120),
                removed: Color32::from_rgb(255, 120, 120),
                changed: Color32::from_rgb(120, 220, 255),
                warning: Color32::from_rgb(255, 230, 0),
                tab_strip: Color32::BLACK,
                radius: 0,
            };
        }
        match theme {
            AppTheme::Pro => Self {
                dark: true,
                chrome: Color32::from_rgb(36, 37, 41),
                background: Color32::from_rgb(29, 30, 33),
                card: Color32::from_rgb(39, 40, 44),
                card_border: Color32::from_rgb(50, 52, 57),
                field: Color32::from_rgb(31, 32, 35),
                field_border: Color32::from_rgb(70, 73, 79),
                hover: Color32::from_rgb(52, 54, 60),
                text: Color32::from_rgb(225, 227, 232),
                text_dim: Color32::from_rgb(148, 152, 160),
                accent: Color32::from_rgb(53, 132, 228),
                accent_text: Color32::WHITE,
                added: Color32::from_rgb(92, 196, 122),
                removed: Color32::from_rgb(238, 94, 102),
                changed: Color32::from_rgb(230, 178, 76),
                warning: Color32::from_rgb(230, 178, 76),
                tab_strip: Color32::from_rgb(36, 37, 41),
                radius: 6,
            },
            AppTheme::ProMedium => Self {
                dark: true,
                chrome: Color32::from_rgb(83, 83, 83),
                background: Color32::from_rgb(66, 66, 66),
                card: Color32::from_rgb(83, 83, 83),
                card_border: Color32::from_rgb(62, 62, 62),
                field: Color32::from_rgb(69, 69, 69),
                field_border: Color32::from_rgb(104, 104, 104),
                hover: Color32::from_rgb(98, 98, 98),
                text: Color32::from_rgb(238, 238, 238),
                text_dim: Color32::from_rgb(200, 200, 200),
                accent: Color32::from_rgb(55, 142, 240),
                accent_text: Color32::WHITE,
                added: Color32::from_rgb(130, 220, 150),
                removed: Color32::from_rgb(255, 130, 135),
                changed: Color32::from_rgb(250, 200, 100),
                warning: Color32::from_rgb(250, 200, 100),
                tab_strip: Color32::from_rgb(74, 74, 74),
                radius: 4,
            },
            AppTheme::Studio => Self {
                dark: true,
                chrome: Color32::from_rgb(20, 20, 21),
                background: Color32::from_rgb(14, 14, 15),
                card: Color32::from_rgb(26, 26, 28),
                card_border: Color32::from_rgb(40, 40, 44),
                field: Color32::from_rgb(35, 35, 38),
                field_border: Color32::from_rgb(64, 64, 70),
                hover: Color32::from_rgb(44, 44, 48),
                text: Color32::from_rgb(236, 236, 240),
                text_dim: Color32::from_rgb(150, 150, 158),
                accent: Color32::from_rgb(139, 124, 246),
                accent_text: Color32::WHITE,
                added: Color32::from_rgb(110, 210, 140),
                removed: Color32::from_rgb(240, 96, 96),
                changed: Color32::from_rgb(240, 190, 90),
                warning: Color32::from_rgb(240, 190, 90),
                tab_strip: Color32::from_rgb(20, 20, 21),
                radius: 8,
            },
            AppTheme::StudioLight => Self {
                dark: false,
                chrome: Color32::from_rgb(250, 249, 253),
                background: Color32::from_rgb(240, 239, 245),
                card: Color32::WHITE,
                card_border: Color32::from_rgb(222, 219, 233),
                field: Color32::from_rgb(244, 243, 249),
                field_border: Color32::from_rgb(196, 191, 214),
                hover: Color32::from_rgb(232, 230, 242),
                text: Color32::from_rgb(33, 31, 43),
                text_dim: Color32::from_rgb(100, 96, 116),
                accent: Color32::from_rgb(112, 92, 236),
                accent_text: Color32::WHITE,
                added: Color32::from_rgb(24, 128, 56),
                removed: Color32::from_rgb(196, 40, 48),
                changed: Color32::from_rgb(160, 100, 0),
                warning: Color32::from_rgb(160, 100, 0),
                tab_strip: Color32::from_rgb(232, 230, 240),
                radius: 8,
            },
        }
    }
}

pub fn apply(ctx: &egui::Context, theme: AppTheme, access: &Access, scale_percent: u32) {
    let p = Palette::get(theme, access);
    let mut visuals = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let radius = CornerRadius::same(p.radius);
    let border = if access.high_contrast { 2.0 } else { 1.0 };
    visuals.panel_fill = p.background;
    visuals.window_fill = p.card;
    visuals.window_stroke = Stroke::new(border, p.card_border);
    visuals.extreme_bg_color = p.field;
    visuals.faint_bg_color = p.hover;
    visuals.code_bg_color = p.field;
    visuals.override_text_color = Some(p.text);
    visuals.hyperlink_color = p.accent;
    visuals.warn_fg_color = p.warning;
    visuals.error_fg_color = p.removed;
    visuals.selection.bg_fill = p.accent.gamma_multiply(if access.high_contrast { 1.0 } else { 0.55 });
    visuals.selection.stroke = Stroke::new(1.0, p.accent_text);
    visuals.window_corner_radius = radius;
    let shadow = egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(if p.dark { 110 } else { 40 }) };
    visuals.popup_shadow = shadow;
    visuals.window_shadow = shadow;
    visuals.menu_corner_radius = radius;
    for (widget, fill) in [
        (&mut visuals.widgets.noninteractive, p.card),
        (&mut visuals.widgets.inactive, p.field),
        (&mut visuals.widgets.hovered, p.hover),
        (&mut visuals.widgets.active, p.accent.gamma_multiply(0.6)),
        (&mut visuals.widgets.open, p.hover),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.corner_radius = radius;
        widget.fg_stroke = Stroke::new(1.0, p.text);
    }
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(border, p.card_border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text_dim);
    visuals.widgets.inactive.bg_stroke = Stroke::new(border, p.field_border);
    visuals.widgets.hovered.bg_stroke = Stroke::new(border.max(1.0), p.accent);
    visuals.widgets.active.bg_stroke = Stroke::new(border.max(1.0), p.accent);
    if access.strong_focus {
        visuals.selection.stroke = Stroke::new(2.5, p.accent);
    }
    let egui_theme = if p.dark { egui::Theme::Dark } else { egui::Theme::Light };
    let text_scale = if access.large_text { 1.2 } else { 1.0 };
    let mut style = (*ctx.style_of(egui_theme)).clone();
    style.visuals = visuals;
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0 * text_scale, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.5 * text_scale, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.5 * text_scale, FontFamily::Name("medium".into()))),
        (TextStyle::Heading, FontId::new(19.0 * text_scale, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.5 * text_scale, FontFamily::Monospace)),
    ]
    .into();
    let targets = if access.big_targets { 1.35 } else { 1.0 };
    style.spacing.item_spacing = egui::vec2(8.0, 7.0 * targets);
    style.spacing.button_padding = egui::vec2(12.0 * targets, 5.0 * targets);
    style.spacing.interact_size = egui::vec2(40.0, 22.0 * targets);
    style.spacing.icon_width = 15.0 * targets;
    style.spacing.icon_width_inner = 9.0 * targets;
    style.animation_time = if access.reduce_motion { 0.0 } else { 0.12 };
    style.scroll_animation = if access.reduce_motion { egui::style::ScrollAnimation::none() } else { egui::style::ScrollAnimation::default() };
    style.url_in_tooltip = true;
    ctx.set_style_of(egui_theme, style);
    ctx.set_theme(egui_theme);
    let magnifier = if access.magnifier { 1.25 } else { 1.0 };
    ctx.set_zoom_factor(scale_percent as f32 / 100.0 * magnifier);
}
