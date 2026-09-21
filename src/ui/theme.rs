//! Visual theme.
//!
//! The default is a true-black ("OLED") palette: on an OLED panel `#000000` pixels
//! are switched off entirely, so the window reads as a floating card rather than a
//! grey box. Everything above the background is built from very low alpha whites,
//! which keeps contrast steps consistent regardless of the accent colour.

use egui::{Color32, CornerRadius, Margin, Stroke, Visuals};

/// Colours derived once per frame-set and handed to the widgets.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    /// Slightly raised surface, used for the header and hovered rows.
    pub surface: Color32,
    pub surface_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    /// Track behind the filled part of a slider.
    pub track: Color32,
    pub danger: Color32,
}

impl Palette {
    pub fn new(oled_black: bool, accent: Color32) -> Self {
        let background = if oled_black {
            Color32::BLACK
        } else {
            Color32::from_rgb(0x12, 0x12, 0x14)
        };

        Self {
            background,
            surface: Color32::from_rgba_unmultiplied(255, 255, 255, 8),
            surface_hover: Color32::from_rgba_unmultiplied(255, 255, 255, 18),
            border: Color32::from_rgb(0x24, 0x24, 0x28),
            text: Color32::from_rgb(0xEC, 0xEC, 0xEF),
            text_dim: Color32::from_rgb(0x8A, 0x8A, 0x95),
            accent,
            track: Color32::from_rgba_unmultiplied(255, 255, 255, 26),
            danger: Color32::from_rgb(0xFF, 0x6B, 0x6B),
        }
    }
}

pub const WINDOW_CORNER: CornerRadius = CornerRadius::same(10);
pub const ROW_CORNER: CornerRadius = CornerRadius::same(8);
pub const CONTENT_MARGIN: Margin = Margin {
    left: 10,
    right: 10,
    top: 8,
    bottom: 10,
};

/// Parse `#rrggbb`. Falls back to the default accent for anything unparseable,
/// so a hand-edited configuration file can never make the UI unreadable.
pub fn parse_accent(text: &str) -> Color32 {
    let hex = text.trim().trim_start_matches('#');

    if hex.len() == 6
        && let Ok(value) = u32::from_str_radix(hex, 16)
    {
        return Color32::from_rgb(
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
        );
    }

    DEFAULT_ACCENT
}

/// Neutral grey, used when the configured accent cannot be parsed.
pub const DEFAULT_ACCENT: Color32 = Color32::from_rgb(0x9A, 0xA3, 0xAD);

pub fn accent_to_hex(color: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
}

/// Accents offered in the settings panel. Chosen to stay legible on pure black.
pub const ACCENT_CHOICES: &[(&str, &str)] = &[
    ("Grey", "#9AA3AD"),
    ("Ice", "#4CC2FF"),
    ("Mint", "#3DDC97"),
    ("Amber", "#FFB454"),
    ("Magenta", "#FF6FB5"),
    ("Violet", "#A88BFF"),
    ("White", "#E8E8EC"),
];

/// Apply the palette to egui's own widget styling, so built-in widgets
/// (scrollbar, tooltips, checkboxes) match the custom ones.
pub fn apply(ctx: &egui::Context, palette: &Palette) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(palette.text);
    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.background;
    visuals.extreme_bg_color = palette.background;
    visuals.faint_bg_color = palette.surface;
    visuals.window_stroke = Stroke::new(1.0, palette.border);
    visuals.window_corner_radius = WINDOW_CORNER;
    visuals.popup_shadow = egui::epaint::Shadow::NONE;
    visuals.window_shadow = egui::epaint::Shadow::NONE;

    visuals.selection.bg_fill = palette.accent.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);

    visuals.widgets.noninteractive.bg_fill = palette.background;
    visuals.widgets.noninteractive.weak_bg_fill = palette.background;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text_dim);

    // Checkboxes and similar built-ins draw their box with `inactive.bg_fill`.
    // On a black background a near-transparent fill made them invisible, so this
    // slot is lifted and given an outline.
    visuals.widgets.inactive.bg_fill = palette.surface_hover;
    visuals.widgets.inactive.weak_bg_fill = palette.surface_hover;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.inactive.corner_radius = ROW_CORNER;

    visuals.widgets.hovered.bg_fill = palette.surface_hover;
    visuals.widgets.hovered.weak_bg_fill = palette.surface_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.hovered.corner_radius = ROW_CORNER;

    visuals.widgets.active.bg_fill = palette.accent.gamma_multiply(0.30);
    visuals.widgets.active.weak_bg_fill = palette.accent.gamma_multiply(0.30);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.active.corner_radius = ROW_CORNER;

    // Applied to both the light and dark style slots: the window is always dark,
    // so following the system theme would only produce an unreadable flash.
    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.spacing.item_spacing = egui::vec2(7.0, 8.0);
        style.spacing.scroll.bar_width = 7.0;
        style.spacing.scroll.floating = true;
        // Interaction feels snappier without egui's default fade animations.
        style.animation_time = 0.06;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_with_and_without_hash() {
        assert_eq!(parse_accent("#FF8800"), Color32::from_rgb(255, 136, 0));
        assert_eq!(parse_accent("ff8800"), Color32::from_rgb(255, 136, 0));
    }

    #[test]
    fn invalid_hex_falls_back_to_default() {
        let fallback = DEFAULT_ACCENT;
        assert_eq!(parse_accent(""), fallback);
        assert_eq!(parse_accent("#xyz"), fallback);
        assert_eq!(parse_accent("#12345"), fallback);
    }

    #[test]
    fn hex_round_trips() {
        for (_, hex) in ACCENT_CHOICES {
            assert_eq!(&accent_to_hex(parse_accent(hex)), hex);
        }
    }

    #[test]
    fn oled_background_is_true_black() {
        let palette = Palette::new(true, Color32::WHITE);
        assert_eq!(palette.background, Color32::BLACK);
        assert_ne!(
            Palette::new(false, Color32::WHITE).background,
            Color32::BLACK
        );
    }
}
