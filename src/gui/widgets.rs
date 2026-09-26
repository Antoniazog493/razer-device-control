/// Synapse-style widgets: toggle, value-bubble slider, cards, preset buttons
/// and the draggable 10-band equalizer.

use eframe::egui::{
    self, emath::remap_clamp, epaint::CubicBezierShape, pos2, vec2, Align2, Color32, CornerRadius,
    CursorIcon, FontId, Margin, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind,
    Ui,
};
use std::ops::RangeInclusive;

use super::theme::*;
use crate::protocol::{EQ_BANDS, EQ_FREQS, EQ_MAX_DB, EQ_MIN_DB};

/// On/off switch.
pub fn toggle(ui: &mut Ui, on: &mut bool, enabled: bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(36.0, 19.0), Sense::click());
    if enabled && resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let r = rect.height() / 2.0;
    let (bg, border, knob) = if !enabled {
        (Color32::from_gray(28), Color32::from_gray(70), Color32::from_gray(80))
    } else if *on {
        (GREEN, GREEN, Color32::from_gray(18))
    } else {
        (Color32::from_gray(26), Color32::from_gray(150), Color32::from_gray(200))
    };
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(r as u8), bg, Stroke::new(1.5, border), StrokeKind::Inside);
    let x = egui::lerp((rect.left() + r)..=(rect.right() - r), t);
    p.circle_filled(pos2(x, rect.center().y), r - 4.0, knob);
    if enabled {
        resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    }
    resp
}

pub struct SliderResponse {
    /// The value moved this frame.
    pub changed: bool,
    /// The interaction finished (drag released or click): time to send.
    pub committed: bool,
}

/// Horizontal slider with a value bubble over the knob and labels under the
/// track, like Synapse's.
pub fn slider(
    ui: &mut Ui,
    value: &mut i32,
    range: RangeInclusive<i32>,
    step: i32,
    labels: (&str, Option<&str>, &str),
    enabled: bool,
    format: impl Fn(i32) -> String,
) -> SliderResponse {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 64.0), Sense::click_and_drag());
    let (min, max) = (*range.start(), *range.end());
    let track_y = rect.top() + 36.0;
    let x0 = rect.left() + 8.0;
    let x1 = rect.right() - 8.0;

    let mut changed = false;
    if resp.is_pointer_button_down_on() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let raw = remap_clamp(pos.x, x0..=x1, min as f32..=max as f32);
            let v = (min + (((raw - min as f32) / step as f32).round() as i32) * step).clamp(min, max);
            if v != *value {
                *value = v;
                changed = true;
            }
        }
    }
    let committed = resp.drag_stopped() || resp.clicked();

    let (fill, knob) = if enabled { (GREEN, GREEN) } else { (GREEN_DIM, GREEN_DIM) };
    let t = if max > min { (*value - min) as f32 / (max - min) as f32 } else { 0.0 };
    let kx = egui::lerp(x0..=x1, t.clamp(0.0, 1.0));
    let p = ui.painter();

    p.line_segment([pos2(x0, track_y), pos2(x1, track_y)], Stroke::new(4.0, TRACK));
    p.line_segment([pos2(x0, track_y), pos2(kx, track_y)], Stroke::new(4.0, fill));
    let r = if resp.hovered() || resp.dragged() { 8.0 } else { 7.0 };
    p.circle_filled(pos2(kx, track_y), r, knob);

    // Value bubble
    let text = format(*value);
    let galley = p.layout_no_wrap(text, FontId::proportional(11.0), Color32::from_gray(15));
    let bw = galley.size().x + 10.0;
    let bubble = Rect::from_center_size(pos2(kx, rect.top() + 11.0), vec2(bw.max(22.0), 18.0));
    let bubble = bubble.translate(vec2(
        (rect.left() - bubble.left()).max(0.0) + (rect.right() - bubble.right()).min(0.0),
        0.0,
    ));
    p.rect_filled(bubble, CornerRadius::same(2), fill);
    p.galley(bubble.center() - galley.size() / 2.0, galley, Color32::from_gray(15));

    let label_y = track_y + 12.0;
    let font = FontId::proportional(11.0);
    let color = if enabled { TEXT_DIM } else { TEXT_FAINT };
    p.text(pos2(x0 - 4.0, label_y), Align2::LEFT_TOP, labels.0, font.clone(), color);
    if let Some(mid) = labels.1 {
        p.text(pos2(rect.center().x, label_y), Align2::CENTER_TOP, mid, font.clone(), color);
    }
    p.text(pos2(x1 + 4.0, label_y), Align2::RIGHT_TOP, labels.2, font, color);

    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    SliderResponse { changed, committed }
}

/// Dark rounded panel that fills the available width.
pub fn card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::symmetric(26, 22))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), add_contents).inner
        })
        .inner
}

/// Card heading: green title, optional toggle next to it and a "?" help icon
/// at the right. Returns true if the toggle was flipped.
pub fn card_title(ui: &mut Ui, title: &str, toggle_state: Option<&mut bool>, help: Option<&str>) -> bool {
    let mut flipped = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(15.0).color(GREEN));
        if let Some(on) = toggle_state {
            ui.add_space(6.0);
            flipped = toggle(ui, on, true).changed();
        }
        if let Some(help) = help {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                help_icon(ui, help);
            });
        }
    });
    ui.add_space(6.0);
    flipped
}

/// Dimmed card heading for features that aren't available.
pub fn card_title_disabled(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).size(15.0).color(GREEN_DIM));
    ui.add_space(6.0);
}

pub fn help_icon(ui: &mut Ui, text: &str) {
    let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    let p = ui.painter();
    p.circle_filled(rect.center(), 7.0, Color32::from_gray(90));
    p.text(rect.center(), Align2::CENTER_CENTER, "?", FontId::proportional(10.0), Color32::from_gray(20));
    resp.on_hover_text(text);
}

pub fn dim_text(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(TEXT_DIM).size(13.0));
}

pub fn faint_text(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(TEXT_FAINT).size(12.0));
}

/// Outlined button used for EQ presets; green outline when selected.
pub fn preset_button(ui: &mut Ui, text: &str, selected: bool, width: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
    let fill = if resp.hovered() { BUTTON_HOVER } else { BUTTON };
    let stroke = if selected {
        Stroke::new(1.5, GREEN)
    } else if resp.hovered() {
        Stroke::new(1.0, Color32::from_gray(120))
    } else {
        Stroke::new(1.0, BORDER)
    };
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(2), fill, stroke, StrokeKind::Inside);
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(13.0),
        if selected { TEXT } else { Color32::from_gray(200) },
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Joined two-or-more-way switch (STANDARD / ESPORTS). Returns true on change.
pub fn segmented(ui: &mut Ui, options: &[&str], selected: &mut usize) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, opt) in options.iter().enumerate() {
            let galley_w = opt.len() as f32 * 8.0 + 28.0;
            let (rect, resp) = ui.allocate_exact_size(vec2(galley_w.max(90.0), 26.0), Sense::click());
            let on = *selected == i;
            let p = ui.painter();
            let fill = if on {
                GREEN
            } else if resp.hovered() {
                BUTTON_HOVER
            } else {
                BUTTON
            };
            p.rect(rect, CornerRadius::ZERO, fill, Stroke::new(1.0, if on { GREEN } else { BORDER }), StrokeKind::Inside);
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                *opt,
                FontId::proportional(12.5),
                if on { Color32::from_gray(10) } else { TEXT },
            );
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() && !on {
                *selected = i;
                changed = true;
            }
        }
    });
    changed
}

/// Underlined text link.
pub fn link(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Label::new(RichText::new(text).underline().color(TEXT)).sense(Sense::click()))
        .on_hover_cursor(CursorIcon::PointingHand)
}

/// Link that opens something outside the app (marked with an arrow).
pub fn external_link(ui: &mut Ui, text: &str) -> Response {
    link(ui, &format!("{text}  ↗"))
}

pub struct EqResponse {
    pub changed: bool,
    /// A drag/click on the graph ended.
    pub committed: bool,
}

const EQ_GROUPS: [(usize, usize, &str); 6] = [
    (0, 0, "SUBGRAVES"),
    (1, 2, "GRAVES"),
    (3, 3, "MED. BAJOS"),
    (4, 5, "MEDIOS"),
    (6, 7, "MED. ALTOS"),
    (8, 9, "AGUDOS"),
];

/// Ten draggable nodes joined by a line, like Synapse's audio equalizer.
/// Read-only (hover shows values) when `editable` is false.
pub fn eq_graph(ui: &mut Ui, bands: &mut [i8; EQ_BANDS], editable: bool) -> EqResponse {
    let width = ui.available_width();
    let sense = if editable { Sense::click_and_drag() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 320.0), sense);
    let plot = Rect::from_min_max(
        pos2(rect.left() + 4.0, rect.top() + 26.0),
        pos2(rect.right() - 64.0, rect.bottom() - 70.0),
    );
    let dx = plot.width() / EQ_BANDS as f32;
    let x_of = |i: usize| plot.left() + dx * (i as f32 + 0.5);
    let y_of = |db: f32| remap_clamp(db, EQ_MAX_DB as f32..=EQ_MIN_DB as f32, plot.top()..=plot.bottom());
    let band_at = |p: Pos2| (((p.x - plot.left()) / dx).floor().max(0.0) as usize).min(EQ_BANDS - 1);

    // The band being dragged sticks until release, even if the pointer
    // wanders sideways.
    let id = resp.id.with("active_band");
    let mut active: Option<usize> = ui.data(|d| d.get_temp(id)).flatten();
    let mut changed = false;
    if resp.is_pointer_button_down_on() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let band = *active.get_or_insert_with(|| band_at(pos));
            let db = remap_clamp(pos.y, plot.top()..=plot.bottom(), EQ_MAX_DB as f32..=EQ_MIN_DB as f32)
                .round() as i8;
            if bands[band] != db {
                bands[band] = db;
                changed = true;
            }
        }
    } else {
        active = None;
    }
    ui.data_mut(|d| d.insert_temp(id, active));
    let committed = resp.drag_stopped() || resp.clicked();
    let highlight = active.or_else(|| resp.hover_pos().map(band_at));
    if editable && resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
    }

    let p = ui.painter_at(rect);

    // dB scale
    for db in [EQ_MAX_DB, 0, EQ_MIN_DB] {
        let label = if db > 0 { format!("+{db}dB") } else { format!("{db}dB") };
        p.text(
            pos2(plot.right() + 22.0, y_of(db as f32)),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.0),
            TEXT,
        );
    }

    // Band lines and zero markers
    for i in 0..EQ_BANDS {
        let x = x_of(i);
        let lit = highlight == Some(i);
        let stroke = if lit { Stroke::new(2.0, GREEN) } else { Stroke::new(1.0, Color32::from_gray(52)) };
        p.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], stroke);
        p.circle_filled(pos2(x, y_of(0.0)), 2.0, Color32::from_rgb(0x3A, 0x6A, 0x30));
        p.text(
            pos2(x, plot.bottom() + 18.0),
            Align2::CENTER_CENTER,
            EQ_FREQS[i],
            FontId::proportional(12.0),
            if lit { GREEN } else { TEXT },
        );
    }

    // Curve and nodes
    let points: Vec<Pos2> = (0..EQ_BANDS).map(|i| pos2(x_of(i), y_of(bands[i] as f32))).collect();
    p.add(Shape::line(points.clone(), Stroke::new(2.0, GREEN_LINE)));
    for (i, &pt) in points.iter().enumerate() {
        let r = if active == Some(i) { 9.0 } else { 7.0 };
        if editable {
            p.circle_filled(pt, r, GREEN);
        } else {
            p.circle_filled(pt, r, CARD);
            p.circle_stroke(pt, r - 1.0, Stroke::new(2.0, GREEN));
        }
    }

    // Value bubble over the dragged/hovered node
    if let Some(i) = highlight {
        let v = bands[i];
        let text = if v > 0 { format!("+{v} dB") } else { format!("{v} dB") };
        let at = points[i] - vec2(0.0, 22.0);
        let galley = p.layout_no_wrap(text, FontId::proportional(11.0), Color32::from_gray(15));
        let bubble = Rect::from_center_size(at, galley.size() + vec2(10.0, 5.0));
        p.rect_filled(bubble, CornerRadius::same(2), GREEN);
        p.galley(bubble.center() - galley.size() / 2.0, galley, Color32::from_gray(15));
    }

    // Frequency region bar
    let bar_top = plot.bottom() + 36.0;
    for (start, end, label) in EQ_GROUPS {
        let r = Rect::from_min_max(
            pos2(x_of(start) - dx / 2.0 + 1.0, bar_top),
            pos2(x_of(end) + dx / 2.0 - 1.0, bar_top + 24.0),
        );
        let lit = highlight.is_some_and(|h| (start..=end).contains(&h));
        p.rect_filled(r, CornerRadius::ZERO, if lit { Color32::from_gray(58) } else { Color32::from_gray(38) });
        p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(10.5), if lit { TEXT } else { TEXT_DIM });
    }

    EqResponse { changed, committed }
}

/// Stylised headset drawing for the Sound tab.
pub fn paint_headset(p: &egui::Painter, c: Pos2, s: f32, lit: bool) {
    let band = Color32::from_gray(38);
    let accent = if lit { GREEN } else { Color32::from_gray(70) };

    // Headband
    let arc = |w: f32, color: Color32| {
        Shape::CubicBezier(CubicBezierShape::from_points_stroke(
            [
                pos2(c.x - 58.0 * s, c.y + 4.0 * s),
                pos2(c.x - 62.0 * s, c.y - 92.0 * s),
                pos2(c.x + 62.0 * s, c.y - 92.0 * s),
                pos2(c.x + 58.0 * s, c.y + 4.0 * s),
            ],
            false,
            Color32::TRANSPARENT,
            Stroke::new(w, color),
        ))
    };
    p.add(arc(11.0 * s, band));
    p.add(arc(1.5 * s, Color32::from_gray(64)));

    // Mic boom (behind the left cup)
    p.add(Shape::CubicBezier(CubicBezierShape::from_points_stroke(
        [
            pos2(c.x - 58.0 * s, c.y + 44.0 * s),
            pos2(c.x - 60.0 * s, c.y + 84.0 * s),
            pos2(c.x - 40.0 * s, c.y + 92.0 * s),
            pos2(c.x - 18.0 * s, c.y + 86.0 * s),
        ],
        false,
        Color32::TRANSPARENT,
        Stroke::new(4.0 * s, Color32::from_gray(64)),
    )));
    p.circle_filled(pos2(c.x - 16.0 * s, c.y + 86.0 * s), 6.0 * s, Color32::from_gray(30));
    p.circle_stroke(pos2(c.x - 16.0 * s, c.y + 86.0 * s), 6.0 * s, Stroke::new(1.0, accent));

    // Ear cups
    for side in [-1.0, 1.0] {
        let cx = c.x + side * 60.0 * s;
        let outer = Rect::from_center_size(pos2(cx, c.y + 28.0 * s), vec2(34.0 * s, 76.0 * s));
        p.rect(
            outer,
            CornerRadius::same((15.0 * s) as u8),
            Color32::from_gray(26),
            Stroke::new(1.0, Color32::from_gray(58)),
            StrokeKind::Inside,
        );
        let inner = outer.shrink2(vec2(7.0 * s, 12.0 * s));
        p.rect_filled(inner, CornerRadius::same((9.0 * s) as u8), Color32::from_gray(16));
        p.circle_filled(outer.center(), 3.5 * s, accent);
    }
}

/// Small horizontal battery gauge.
pub fn battery_icon(ui: &mut Ui, level: Option<u8>, charging: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(26.0, 13.0), Sense::hover());
    let p = ui.painter();
    let body = Rect::from_min_max(rect.min, pos2(rect.right() - 3.0, rect.bottom()));
    p.rect_stroke(body, CornerRadius::same(2), Stroke::new(1.2, TEXT), StrokeKind::Inside);
    p.rect_filled(
        Rect::from_center_size(pos2(rect.right() - 1.5, rect.center().y), vec2(2.5, 5.0)),
        CornerRadius::same(1),
        TEXT,
    );
    if let Some(level) = level {
        let color = if charging {
            GREEN
        } else if level <= 15 {
            ERROR
        } else {
            TEXT
        };
        let inner = body.shrink(2.5);
        let w = inner.width() * (level as f32 / 100.0);
        p.rect_filled(
            Rect::from_min_size(inner.min, vec2(w.max(1.0), inner.height())),
            CornerRadius::same(1),
            color,
        );
    }
}
