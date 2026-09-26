/// Colours and base style, modelled on Razer Synapse 4.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub const GREEN: Color32 = Color32::from_rgb(0x44, 0xD6, 0x2C);
pub const GREEN_DIM: Color32 = Color32::from_rgb(0x2C, 0x6E, 0x22);
pub const GREEN_LINE: Color32 = Color32::from_rgb(0x2E, 0x7D, 0x22);
pub const BG: Color32 = Color32::from_rgb(0x22, 0x22, 0x22);
pub const CARD: Color32 = Color32::from_rgb(0x11, 0x11, 0x11);
pub const TITLEBAR: Color32 = Color32::from_rgb(0x0B, 0x0B, 0x0B);
pub const FOOTER: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2B);
pub const BUTTON: Color32 = Color32::from_rgb(0x16, 0x16, 0x16);
pub const BUTTON_HOVER: Color32 = Color32::from_rgb(0x21, 0x21, 0x21);
pub const BORDER: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
pub const TRACK: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3A);
pub const TEXT: Color32 = Color32::from_rgb(0xE4, 0xE4, 0xE4);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9A, 0x9A, 0x9A);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x62, 0x62, 0x62);
pub const WARN: Color32 = Color32::from_rgb(0xF0, 0xA0, 0x30);
pub const ERROR: Color32 = Color32::from_rgb(0xE8, 0x4A, 0x4A);

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |s| {
        s.text_styles = [
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();

        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        s.spacing.combo_width = 220.0;

        let v = &mut s.visuals;
        v.override_text_color = Some(TEXT);
        v.panel_fill = BG;
        v.window_fill = CARD;
        v.window_stroke = Stroke::new(1.0, Color32::from_gray(50));
        v.window_corner_radius = CornerRadius::same(4);
        v.menu_corner_radius = CornerRadius::same(3);
        v.extreme_bg_color = Color32::from_gray(10);
        v.faint_bg_color = Color32::from_gray(24);
        v.hyperlink_color = TEXT;
        v.selection.bg_fill = GREEN_DIM;
        v.selection.stroke = Stroke::new(1.0, GREEN);

        let w = &mut v.widgets;
        for state in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
            state.corner_radius = CornerRadius::same(2);
            state.expansion = 0.0;
        }
        w.inactive.bg_fill = BUTTON;
        w.inactive.weak_bg_fill = BUTTON;
        w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        w.hovered.bg_fill = BUTTON_HOVER;
        w.hovered.weak_bg_fill = BUTTON_HOVER;
        w.hovered.bg_stroke = Stroke::new(1.0, GREEN);
        w.active.bg_fill = GREEN_DIM;
        w.active.weak_bg_fill = GREEN_DIM;
        w.active.bg_stroke = Stroke::new(1.0, GREEN);
        w.open.bg_fill = BUTTON_HOVER;
        w.open.weak_bg_fill = BUTTON_HOVER;
        w.open.bg_stroke = Stroke::new(1.0, GREEN);
    });
}
