//! Vector icons drawn with the painter.
//!
//! Emoji glyphs were the obvious first choice, but the bundled emoji font only
//! covers part of what a mixer needs and the rest renders as tofu boxes. Drawing
//! them keeps every icon crisp at any DPI and consistent with the custom widgets.
//!
//! All shapes are authored on a 16×16 grid and mapped into the target rectangle.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Close,
    /// Three sliders; reads as "settings" and suits a volume app better than a gear.
    Settings,
    Pin,
    /// Circular arrow: apply the saved values.
    Sync,
    /// Arrow into a tray: store the current values.
    Save,
    Speaker,
    SpeakerMuted,
    Trash,
}

/// Side of the design grid.
const GRID: f32 = 16.0;
/// Fraction of the button the icon occupies.
const SCALE: f32 = 0.66;

struct Canvas {
    origin: Pos2,
    unit: f32,
}

impl Canvas {
    fn new(rect: Rect) -> Self {
        let size = rect.width().min(rect.height()) * SCALE;
        Self {
            origin: rect.center() - vec2(size / 2.0, size / 2.0),
            unit: size / GRID,
        }
    }

    /// Grid coordinates to screen coordinates.
    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.origin.x + x * self.unit, self.origin.y + y * self.unit)
    }

    fn length(&self, value: f32) -> f32 {
        value * self.unit
    }
}

/// `background` is used to punch a gap where a shape overlaps another, so the
/// icon stays legible against whatever the button is filled with.
pub fn draw(painter: &Painter, rect: Rect, icon: Icon, color: Color32, background: Color32) {
    let canvas = Canvas::new(rect);
    // Scale the stroke with the icon so it stays proportional when zoomed.
    let stroke = Stroke::new(canvas.length(1.35).max(1.0), color);

    match icon {
        Icon::Close => {
            painter.line_segment([canvas.at(4.0, 4.0), canvas.at(12.0, 12.0)], stroke);
            painter.line_segment([canvas.at(12.0, 4.0), canvas.at(4.0, 12.0)], stroke);
        }

        Icon::Settings => {
            // Two sliders rather than three: at button size three rows sit so close
            // that the knobs eat the lines and the whole icon reads as noise.
            let line = Stroke::new(canvas.length(1.2).max(1.0), color);
            let knob_radius = canvas.length(2.4);

            for (y, knob_x) in [(5.5, 10.5), (10.5, 5.5)] {
                painter.line_segment([canvas.at(2.0, y), canvas.at(14.0, y)], line);
                painter.circle(
                    canvas.at(knob_x, y),
                    knob_radius,
                    color,
                    Stroke::new(canvas.length(1.3), background),
                );
            }
        }

        Icon::Pin => {
            painter.circle_filled(canvas.at(8.0, 5.5), canvas.length(3.1), color);
            painter.line_segment([canvas.at(8.0, 8.6), canvas.at(8.0, 14.0)], stroke);
        }

        Icon::Sync => {
            // Three quarters of a circle with an arrow head on the leading end,
            // so it reads as "apply again" rather than as a plain ring.
            let centre = canvas.at(8.0, 8.0);
            let radius = canvas.length(5.2);

            let start = 70.0_f32;
            let sweep = 285.0_f32;
            let steps = 40;

            let points: Vec<Pos2> = (0..=steps)
                .map(|step| {
                    let angle = (start + sweep * step as f32 / steps as f32).to_radians();
                    pos2(
                        centre.x + radius * angle.cos(),
                        centre.y + radius * angle.sin(),
                    )
                })
                .collect();

            painter.add(Shape::line(points, stroke));

            // Head sits at the end of the sweep and points along the tangent,
            // which for an increasing angle is (-sin, cos).
            let end = (start + sweep).to_radians();
            let tip_base = pos2(centre.x + radius * end.cos(), centre.y + radius * end.sin());
            let tangent = egui::vec2(-end.sin(), end.cos());
            let normal = egui::vec2(-tangent.y, tangent.x);
            let size = canvas.length(3.0);

            painter.add(Shape::convex_polygon(
                vec![
                    tip_base + tangent * size,
                    tip_base + normal * size * 0.62,
                    tip_base - normal * size * 0.62,
                ],
                color,
                Stroke::NONE,
            ));
        }

        Icon::Save => {
            painter.line_segment([canvas.at(8.0, 2.5), canvas.at(8.0, 9.0)], stroke);
            painter.add(Shape::convex_polygon(
                vec![
                    canvas.at(8.0, 12.0),
                    canvas.at(4.6, 8.0),
                    canvas.at(11.4, 8.0),
                ],
                color,
                Stroke::NONE,
            ));
            painter.line_segment([canvas.at(3.0, 14.0), canvas.at(13.0, 14.0)], stroke);
        }

        Icon::Speaker | Icon::SpeakerMuted => {
            // Cone: a rectangle at the left joined to a triangle opening right.
            painter.add(Shape::convex_polygon(
                vec![
                    canvas.at(2.5, 6.2),
                    canvas.at(5.2, 6.2),
                    canvas.at(8.6, 2.8),
                    canvas.at(8.6, 13.2),
                    canvas.at(5.2, 9.8),
                    canvas.at(2.5, 9.8),
                ],
                color,
                Stroke::NONE,
            ));

            if icon == Icon::Speaker {
                // Two sound waves as arcs to the right of the cone.
                for (radius_units, spread) in [(2.4_f32, 0.85_f32), (4.2, 0.75)] {
                    let centre = canvas.at(8.6, 8.0);
                    let radius = canvas.length(radius_units);

                    let points: Vec<Pos2> = (0..=14)
                        .map(|step| {
                            let t = step as f32 / 14.0;
                            let angle = -spread + t * 2.0 * spread;
                            pos2(
                                centre.x + radius * angle.cos(),
                                centre.y + radius * angle.sin(),
                            )
                        })
                        .collect();

                    painter.add(Shape::line(
                        points,
                        Stroke::new(canvas.length(1.2).max(1.0), color),
                    ));
                }
            } else {
                let cross = Stroke::new(canvas.length(1.4).max(1.0), color);
                painter.line_segment([canvas.at(10.4, 5.8), canvas.at(14.2, 10.2)], cross);
                painter.line_segment([canvas.at(14.2, 5.8), canvas.at(10.4, 10.2)], cross);
            }
        }

        Icon::Trash => {
            painter.line_segment([canvas.at(2.8, 4.6), canvas.at(13.2, 4.6)], stroke);

            // Lid handle.
            painter.add(Shape::line(
                vec![
                    canvas.at(6.3, 4.6),
                    canvas.at(6.3, 2.6),
                    canvas.at(9.7, 2.6),
                    canvas.at(9.7, 4.6),
                ],
                stroke,
            ));

            // Body.
            painter.add(Shape::line(
                vec![
                    canvas.at(4.3, 4.6),
                    canvas.at(5.1, 13.6),
                    canvas.at(10.9, 13.6),
                    canvas.at(11.7, 4.6),
                ],
                stroke,
            ));
        }
    }
}
