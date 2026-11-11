//! Colour themes for the whole bench: sheet, symbols, panels and instruments.

use egui::{epaint::Shadow, Color32, Rounding, Stroke, Visuals};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeKind {
    Light,
    Dark,
    Midnight,
    Blueprint,
    Sepia,
    Forest,
}

impl ThemeKind {
    pub const ALL: [ThemeKind; 6] = [ThemeKind::Light, ThemeKind::Dark, ThemeKind::Midnight, ThemeKind::Blueprint, ThemeKind::Sepia, ThemeKind::Forest];
    pub fn name(self) -> &'static str {
        match self {
            ThemeKind::Light => "Light",
            ThemeKind::Dark => "Dark",
            ThemeKind::Midnight => "Midnight",
            ThemeKind::Blueprint => "Blueprint",
            ThemeKind::Sepia => "Sepia",
            ThemeKind::Forest => "Forest",
        }
    }
    pub fn palette(self) -> Palette {
        match self {
            ThemeKind::Light => LIGHT,
            ThemeKind::Dark => DARK,
            ThemeKind::Midnight => MIDNIGHT,
            ThemeKind::Blueprint => BLUEPRINT,
            ThemeKind::Sepia => SEPIA,
            ThemeKind::Forest => FOREST,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub sheet: Color32,
    pub grid: Color32,
    pub ink: Color32,
    pub label: Color32,
    pub value: Color32,
    pub select: Color32,
    pub box_fill: Color32,
    pub box_fill2: Color32,
    pub panel: Color32,
    pub window: Color32,
    pub widget: Color32,
    pub accent: Color32,
    pub text: Color32,
    pub weak_text: Color32,
    pub lcd_bg: Color32,
    pub lcd_text: Color32,
    pub flow: Color32,
    pub junction: Color32,
}

const fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub const LIGHT: Palette = Palette {
    dark: false,
    sheet: c(255, 255, 255),
    grid: c(196, 202, 216),
    ink: c(28, 32, 110),
    label: c(20, 70, 170),
    value: c(70, 70, 80),
    select: c(0, 140, 255),
    box_fill: c(240, 243, 250),
    box_fill2: c(222, 228, 242),
    panel: c(234, 237, 244),
    window: c(246, 248, 252),
    widget: c(222, 227, 238),
    accent: c(52, 120, 246),
    text: c(30, 32, 40),
    weak_text: c(110, 115, 130),
    lcd_bg: c(200, 220, 176),
    lcd_text: c(20, 34, 20),
    flow: c(255, 170, 0),
    junction: c(200, 30, 30),
};

pub const DARK: Palette = Palette {
    dark: true,
    sheet: c(28, 30, 36),
    grid: c(62, 66, 78),
    ink: c(150, 190, 255),
    label: c(120, 200, 255),
    value: c(190, 195, 210),
    select: c(255, 190, 60),
    box_fill: c(44, 48, 58),
    box_fill2: c(36, 39, 48),
    panel: c(36, 38, 45),
    window: c(42, 45, 53),
    widget: c(56, 60, 72),
    accent: c(90, 150, 255),
    text: c(225, 228, 235),
    weak_text: c(140, 146, 160),
    lcd_bg: c(18, 40, 30),
    lcd_text: c(110, 255, 160),
    flow: c(255, 210, 60),
    junction: c(255, 90, 80),
};

pub const MIDNIGHT: Palette = Palette {
    dark: true,
    sheet: c(14, 20, 38),
    grid: c(40, 54, 92),
    ink: c(180, 160, 255),
    label: c(255, 150, 220),
    value: c(180, 190, 230),
    select: c(0, 230, 200),
    box_fill: c(30, 38, 70),
    box_fill2: c(24, 30, 58),
    panel: c(20, 26, 48),
    window: c(26, 33, 60),
    widget: c(42, 52, 92),
    accent: c(150, 110, 255),
    text: c(225, 228, 250),
    weak_text: c(140, 150, 200),
    lcd_bg: c(10, 18, 44),
    lcd_text: c(120, 220, 255),
    flow: c(0, 240, 255),
    junction: c(255, 110, 180),
};

pub const BLUEPRINT: Palette = Palette {
    dark: true,
    sheet: c(18, 62, 120),
    grid: c(60, 110, 170),
    ink: c(235, 245, 255),
    label: c(255, 240, 160),
    value: c(200, 225, 255),
    select: c(255, 200, 0),
    box_fill: c(30, 80, 145),
    box_fill2: c(24, 70, 132),
    panel: c(20, 50, 92),
    window: c(24, 58, 104),
    widget: c(36, 78, 136),
    accent: c(255, 200, 60),
    text: c(235, 242, 255),
    weak_text: c(160, 190, 225),
    lcd_bg: c(10, 36, 70),
    lcd_text: c(255, 230, 120),
    flow: c(255, 230, 90),
    junction: c(255, 230, 120),
};

pub const SEPIA: Palette = Palette {
    dark: false,
    sheet: c(250, 244, 230),
    grid: c(214, 198, 170),
    ink: c(90, 56, 30),
    label: c(150, 70, 30),
    value: c(110, 90, 70),
    select: c(210, 110, 20),
    box_fill: c(242, 232, 210),
    box_fill2: c(230, 216, 190),
    panel: c(238, 228, 208),
    window: c(248, 240, 224),
    widget: c(226, 212, 186),
    accent: c(190, 110, 40),
    text: c(60, 44, 30),
    weak_text: c(130, 110, 90),
    lcd_bg: c(214, 206, 160),
    lcd_text: c(50, 40, 20),
    flow: c(230, 120, 20),
    junction: c(170, 50, 30),
};

pub const FOREST: Palette = Palette {
    dark: true,
    sheet: c(20, 32, 26),
    grid: c(48, 72, 58),
    ink: c(160, 230, 170),
    label: c(240, 220, 130),
    value: c(190, 215, 195),
    select: c(255, 150, 90),
    box_fill: c(34, 52, 42),
    box_fill2: c(28, 44, 36),
    panel: c(26, 38, 32),
    window: c(32, 46, 38),
    widget: c(46, 66, 54),
    accent: c(110, 200, 120),
    text: c(225, 238, 228),
    weak_text: c(140, 170, 150),
    lcd_bg: c(14, 30, 20),
    lcd_text: c(160, 255, 150),
    flow: c(255, 220, 90),
    junction: c(255, 140, 90),
};

pub static PALETTE: RwLock<Palette> = RwLock::new(LIGHT);

/// The palette in use.
pub fn pal() -> Palette {
    *PALETTE.read().unwrap()
}

/// Switch theme: palette for drawing plus matching egui visuals.
pub fn apply(ctx: &egui::Context, kind: ThemeKind) {
    let p = kind.palette();
    *PALETTE.write().unwrap() = p;
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.panel;
    v.window_fill = p.window;
    v.extreme_bg_color = if p.dark { p.box_fill2 } else { Color32::WHITE };
    v.faint_bg_color = p.box_fill;
    v.code_bg_color = p.box_fill2;
    v.hyperlink_color = p.accent;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent.linear_multiply(if p.dark { 0.55 } else { 0.35 });
    v.selection.stroke = Stroke::new(1.0, p.text);
    v.window_rounding = Rounding::same(10.0);
    v.menu_rounding = Rounding::same(8.0);
    v.window_stroke = Stroke::new(1.0, p.grid);
    v.window_shadow = Shadow { offset: egui::vec2(0.0, 8.0), blur: 24.0, spread: 0.0, color: Color32::from_black_alpha(if p.dark { 140 } else { 50 }) };
    v.popup_shadow = Shadow { offset: egui::vec2(0.0, 4.0), blur: 12.0, spread: 0.0, color: Color32::from_black_alpha(if p.dark { 120 } else { 40 }) };
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.rounding = Rounding::same(6.0);
    }
    v.widgets.noninteractive.bg_fill = p.panel;
    v.widgets.noninteractive.weak_bg_fill = p.panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.grid.linear_multiply(0.8));
    v.widgets.inactive.weak_bg_fill = p.widget;
    v.widgets.inactive.bg_fill = p.widget;
    v.widgets.hovered.weak_bg_fill = blend(p.widget, p.accent, 0.25);
    v.widgets.hovered.bg_fill = blend(p.widget, p.accent, 0.25);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.active.weak_bg_fill = blend(p.widget, p.accent, 0.45);
    v.widgets.active.bg_fill = blend(p.widget, p.accent, 0.45);
    ctx.set_visuals(v);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.animation_time = 0.12;
    ctx.set_style(style);
}

pub fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Wire colour adjusted so it stays readable on the current sheet.
pub fn wire_color(rgb: [u8; 3]) -> Color32 {
    let p = pal();
    let col = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
    if p.dark {
        if rgb == [30, 30, 30] {
            return Color32::from_gray(225);
        }
        blend(col, Color32::WHITE, 0.25)
    } else {
        col
    }
}
