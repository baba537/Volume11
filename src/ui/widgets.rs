//! Custom widgets.
//!
//! egui's stock `Slider` carries a drag-value box and tick styling that does not
//! suit a volume mixer. These are drawn directly, which is both faster (a handful
//! of shapes, no layout nesting) and gives exact control over the look.

use egui::{
    Align, Align2, Color32, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2,
};

use super::icons::{self, Icon};
use super::theme::{Palette, ROW_CORNER};

// Deliberately chunky: the point of the slider is that it can be grabbed and
// dragged without aiming carefully.
const TRACK_HEIGHT: f32 = 10.0;
const KNOB_RADIUS: f32 = 10.0;
const KNOB_RADIUS_ACTIVE: f32 = 11.5;
/// Extra vertical room so the whole strip responds, not just the track itself.
const HIT_PADDING: f32 = 5.0;

/// A horizontal volume slider over 0..=100.
///
/// Returns a response whose `changed()` is true only while the value actually
/// moves, so the caller can send one command per change instead of per frame.
pub fn volume_slider(ui: &mut Ui, value: &mut u8, palette: &Palette, enabled: bool) -> Response {
    let width = ui.available_width();
    let (rect, mut response) = ui.allocate_exact_size(
        Vec2::new(width, KNOB_RADIUS_ACTIVE * 2.0 + HIT_PADDING * 2.0),
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );

    // Inset by the knob radius so the knob never overhangs the widget bounds.
    let track_left = rect.left() + KNOB_RADIUS_ACTIVE;
    let track_right = rect.right() - KNOB_RADIUS_ACTIVE;
    let track_width = (track_right - track_left).max(1.0);
    let centre_y = rect.center().y;

    if enabled && let Some(pointer) = response.interact_pointer_pos() {
        // Jump straight to the clicked position, like the Windows flyout does.
        let ratio = ((pointer.x - track_left) / track_width).clamp(0.0, 1.0);
        let new_value = (ratio * 100.0).round() as u8;

        if new_value != *value {
            *value = new_value;
            response.mark_changed();
        }
    }

    if !ui.is_rect_visible(rect) {
        return response;
    }

    let ratio = f32::from(*value) / 100.0;
    let knob_x = track_left + track_width * ratio;
    let painter = ui.painter();

    let dimmed = if enabled { 1.0 } else { 0.45 };

    // Unfilled track.
    painter.rect_filled(
        Rect::from_min_max(
            pos2(track_left, centre_y - TRACK_HEIGHT / 2.0),
            pos2(track_right, centre_y + TRACK_HEIGHT / 2.0),
        ),
        TRACK_HEIGHT / 2.0,
        palette.track.gamma_multiply(dimmed),
    );

    // Filled portion.
    if knob_x > track_left {
        painter.rect_filled(
            Rect::from_min_max(
                pos2(track_left, centre_y - TRACK_HEIGHT / 2.0),
                pos2(knob_x, centre_y + TRACK_HEIGHT / 2.0),
            ),
            TRACK_HEIGHT / 2.0,
            palette.accent.gamma_multiply(dimmed),
        );
    }

    let radius = if response.dragged() || response.hovered() {
        KNOB_RADIUS_ACTIVE
    } else {
        KNOB_RADIUS
    };

    painter.circle(
        pos2(knob_x, centre_y),
        radius * if enabled { 1.0 } else { 0.85 },
        palette.accent.gamma_multiply(dimmed),
        Stroke::new(2.0, palette.background),
    );

    response
}

/// Small square icon button used for mute, settings and the window controls.
pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    tooltip: &str,
    active: bool,
    palette: &Palette,
) -> Response {
    let size = Vec2::splat(30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        let fill = if active {
            palette.accent.gamma_multiply(0.22)
        } else if response.hovered() {
            palette.surface_hover
        } else {
            Color32::TRANSPARENT
        };

        if fill != Color32::TRANSPARENT {
            painter.rect_filled(rect, ROW_CORNER, fill);
        }

        if active {
            painter.rect_stroke(
                rect,
                ROW_CORNER,
                Stroke::new(1.0, palette.accent.gamma_multiply(0.7)),
                StrokeKind::Inside,
            );
        }

        let colour = if active {
            palette.accent
        } else if response.hovered() {
            palette.text
        } else {
            palette.text_dim
        };

        icons::draw(painter, rect, icon, colour, palette.background);
    }

    response.on_hover_text(tooltip)
}

/// Text button with a subtle surface, used in the settings panel.
pub fn chip_button(ui: &mut Ui, label: &str, active: bool, palette: &Palette) -> Response {
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), FontId::proportional(14.0), palette.text);

    let size = Vec2::new(galley.size().x + 22.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        let fill = if active {
            palette.accent.gamma_multiply(0.22)
        } else if response.hovered() {
            palette.surface_hover
        } else {
            palette.surface
        };

        painter.rect_filled(rect, ROW_CORNER, fill);

        if active {
            painter.rect_stroke(
                rect,
                ROW_CORNER,
                Stroke::new(1.0, palette.accent.gamma_multiply(0.8)),
                StrokeKind::Inside,
            );
        }

        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(14.0),
            if active { palette.accent } else { palette.text },
        );
    }

    response
}

/// A round colour swatch for picking the accent.
pub fn colour_swatch(ui: &mut Ui, colour: Color32, active: bool, palette: &Palette) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.circle_filled(rect.center(), 9.5, colour);

        if active || response.hovered() {
            painter.circle_stroke(
                rect.center(),
                12.0,
                Stroke::new(1.5, if active { colour } else { palette.text_dim }),
            );
        }
    }

    response
}

/// Height of the editable percentage field. Generous on purpose: the old
/// interface had a cramped 30×20 box that was awkward to hit.
const NUMBER_FIELD_HEIGHT: f32 = 30.0;

/// Editable percentage box.
///
/// Returns the new value while the typed text parses to something different.
/// `editing` holds the raw text of whichever field currently has focus, so a
/// half-typed number is not overwritten by the value it is about to become.
pub fn number_field(
    ui: &mut Ui,
    value: u8,
    key: &str,
    editing: &mut Option<(String, String)>,
    palette: &Palette,
    width: f32,
) -> Option<u8> {
    let mut buffer = match editing {
        Some((editing_key, text)) if editing_key == key => text.clone(),
        _ => value.to_string(),
    };

    let response = ui.add_sized(
        Vec2::new(width, NUMBER_FIELD_HEIGHT),
        egui::TextEdit::singleline(&mut buffer)
            .font(FontId::proportional(15.0))
            .horizontal_align(Align::Center)
            .margin(egui::Margin::symmetric(2, 4))
            .text_color(palette.text),
    );

    if response.gained_focus() {
        *editing = Some((key.to_string(), value.to_string()));
        return None;
    }

    if response.lost_focus() {
        *editing = None;
        return None;
    }

    if !response.changed() {
        return None;
    }

    // Discard anything that is not a digit rather than blanking the field, which
    // is what the old interface did and what made it fight the user mid-edit.
    let digits: String = buffer
        .chars()
        .filter(char::is_ascii_digit)
        .take(3)
        .collect();

    *editing = Some((key.to_string(), digits.clone()));

    // An empty field is a valid intermediate state; nothing is applied yet.
    digits
        .parse::<u32>()
        .ok()
        .map(|parsed| parsed.min(100) as u8)
}

/// The current output device, as a row that unfolds the device list.
///
/// Drawn as one wide target with a chevron on the right, so it reads as
/// something to click rather than a caption. Only offered as a control when
/// there is more than one device to choose from.
pub fn device_selector(
    ui: &mut Ui,
    name: &str,
    open: bool,
    switchable: bool,
    palette: &Palette,
) -> Response {
    let width = ui.available_width();
    let height = 26.0;
    let sense = if switchable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), sense);

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        if switchable && (response.hovered() || open) {
            painter.rect_filled(rect, ROW_CORNER, palette.surface_hover);
        }

        let chevron_space = if switchable { 22.0 } else { 0.0 };
        let text_rect = Rect::from_min_max(
            pos2(rect.left() + 6.0, rect.top()),
            pos2(rect.right() - chevron_space, rect.bottom()),
        );

        let colour = if switchable && response.hovered() {
            palette.text
        } else {
            palette.text_dim
        };

        let galley = painter.layout(
            name.to_string(),
            FontId::proportional(12.5),
            colour,
            f32::INFINITY,
        );

        // Clip rather than wrap: a long device name must stay on one line.
        painter.with_clip_rect(text_rect).galley(
            pos2(text_rect.left(), rect.center().y - galley.size().y / 2.0),
            galley,
            colour,
        );

        if switchable {
            let centre = pos2(rect.right() - 12.0, rect.center().y);
            let size = 4.0;
            // Points down when folded, up when the list is open.
            let points = if open {
                vec![
                    pos2(centre.x - size, centre.y + size / 2.0),
                    pos2(centre.x + size, centre.y + size / 2.0),
                    pos2(centre.x, centre.y - size / 2.0),
                ]
            } else {
                vec![
                    pos2(centre.x - size, centre.y - size / 2.0),
                    pos2(centre.x + size, centre.y - size / 2.0),
                    pos2(centre.x, centre.y + size / 2.0),
                ]
            };
            painter.add(egui::Shape::convex_polygon(points, colour, Stroke::NONE));
        }
    }

    response
}

/// One entry in the unfolded device list.
pub fn device_option(ui: &mut Ui, name: &str, current: bool, palette: &Palette) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        if current {
            painter.rect_filled(rect, ROW_CORNER, palette.accent.gamma_multiply(0.16));
        } else if response.hovered() {
            painter.rect_filled(rect, ROW_CORNER, palette.surface_hover);
        }

        // A dot marks the device in use, so the list works without colour too.
        if current {
            painter.circle_filled(
                pos2(rect.left() + 14.0, rect.center().y),
                3.5,
                palette.accent,
            );
        }

        let text_rect = Rect::from_min_max(
            pos2(rect.left() + 26.0, rect.top()),
            pos2(rect.right() - 8.0, rect.bottom()),
        );
        let colour = if current {
            palette.accent
        } else {
            palette.text
        };
        let galley = painter.layout(
            name.to_string(),
            FontId::proportional(14.0),
            colour,
            f32::INFINITY,
        );
        painter.with_clip_rect(text_rect).galley(
            pos2(text_rect.left(), rect.center().y - galley.size().y / 2.0),
            galley,
            colour,
        );
    }

    response.on_hover_text(name)
}
