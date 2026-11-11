//! Drawing schematic symbols with egui's painter (colours follow the active theme).

use crate::model::*;
use crate::theme::{blend, pal};
use egui::{pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};

/// Live state shown on a symbol while simulating.
#[derive(Clone, Default)]
pub struct Live {
    /// Displayed (smoothed) brightness / activity 0..1.
    pub glow: f32,
    /// Target brightness the display eases towards.
    pub glow_target: f32,
    /// Rotor angle for motors (radians).
    pub spin: f32,
    /// Free-running animation clock (seconds).
    pub time: f32,
    pub reading: Option<String>,
}

pub struct Pen<'a> {
    pub painter: &'a Painter,
    pub origin: Pos2,
    pub s: f32,
    pub rot: u8,
    pub stroke: Stroke,
}

impl<'a> Pen<'a> {
    pub fn p(&self, x: f32, y: f32) -> Pos2 {
        let (rx, ry) = rotate_f(x, y, self.rot);
        self.origin + vec2(rx * self.s, ry * self.s)
    }
    pub fn line(&self, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.painter.line_segment([self.p(x1, y1), self.p(x2, y2)], self.stroke);
    }
    pub fn line_w(&self, x1: f32, y1: f32, x2: f32, y2: f32, w: f32) {
        self.painter.line_segment([self.p(x1, y1), self.p(x2, y2)], Stroke::new(self.stroke.width * w, self.stroke.color));
    }
    pub fn line_c(&self, x1: f32, y1: f32, x2: f32, y2: f32, col: Color32) {
        self.painter.line_segment([self.p(x1, y1), self.p(x2, y2)], Stroke::new(self.stroke.width, col));
    }
    pub fn poly(&self, pts: &[(f32, f32)], closed: bool) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.p(x, y)).collect();
        if closed {
            self.painter.add(Shape::closed_line(v, self.stroke));
        } else {
            self.painter.add(Shape::line(v, self.stroke));
        }
    }
    pub fn fill_poly(&self, pts: &[(f32, f32)], fill: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter.add(Shape::convex_polygon(v, fill, self.stroke));
    }
    pub fn circle(&self, x: f32, y: f32, r: f32) {
        self.painter.circle_stroke(self.p(x, y), r * self.s, self.stroke);
    }
    pub fn fill_circle(&self, x: f32, y: f32, r: f32, fill: Color32) {
        self.painter.circle_filled(self.p(x, y), r * self.s, fill);
    }
    pub fn rect_screen(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_two_pos(self.p(x0, y0), self.p(x1, y1))
    }
    /// Rounded, softly shaded box (instrument bodies, meters).
    pub fn panel(&self, x0: f32, y0: f32, x1: f32, y1: f32, fill: Color32) {
        let r = self.rect_screen(x0, y0, x1, y1);
        let rad = (0.35 * self.s).clamp(2.0, 10.0);
        // soft drop shadow
        let sh = Color32::from_black_alpha(if pal().dark { 90 } else { 30 });
        self.painter.rect_filled(r.translate(vec2(0.0, 0.18 * self.s)).expand(0.5), rad, sh);
        self.painter.rect_filled(r, rad, fill);
        // top highlight band
        let top = Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + r.height() * 0.42));
        self.painter.rect_filled(top.shrink(1.0), rad, Color32::from_white_alpha(if pal().dark { 10 } else { 70 }));
        self.painter.rect_stroke(r, rad, self.stroke);
    }
    pub fn text(&self, x: f32, y: f32, t: &str, size: f32, color: Color32, align: Align2) {
        let fs = (size * self.s).clamp(5.0, 40.0);
        self.painter.text(self.p(x, y), align, t, FontId::proportional(fs), color);
    }
    /// Text placed at a rotated anchor but never rotated itself.
    pub fn text_fixed(&self, pos: Pos2, t: &str, size: f32, color: Color32, align: Align2) {
        let fs = (size * self.s).clamp(5.0, 40.0);
        self.painter.text(pos, align, t, FontId::proportional(fs), color);
    }
}

fn arrow_head(pen: &Pen, tip: (f32, f32), from: (f32, f32), size: f32) {
    let (dx, dy) = (tip.0 - from.0, tip.1 - from.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    let (ux, uy) = (dx / len, dy / len);
    let (px, py) = (-uy, ux);
    let b = (tip.0 - ux * size, tip.1 - uy * size);
    pen.fill_poly(&[tip, (b.0 + px * size * 0.5, b.1 + py * size * 0.5), (b.0 - px * size * 0.5, b.1 - py * size * 0.5)], pen.stroke.color);
}

fn sine_glyph(pen: &Pen, cx: f32, cy: f32, w: f32, h: f32) {
    let mut pts = Vec::new();
    for i in 0..=24 {
        let t = i as f32 / 24.0;
        pts.push((cx - w / 2.0 + w * t, cy - h / 2.0 * (t * std::f32::consts::TAU).sin()));
    }
    pen.poly(&pts, false);
}

fn square_glyph(pen: &Pen, cx: f32, cy: f32, w: f32, h: f32) {
    let (x0, x1, y0, y1) = (cx - w / 2.0, cx + w / 2.0, cy - h / 2.0, cy + h / 2.0);
    let m = cx;
    pen.poly(&[(x0, y1), (x0, y0), (m, y0), (m, y1), (x1, y1), (x1, y0)], false);
}

fn plus(pen: &Pen, x: f32, y: f32, r: f32) {
    pen.line_w(x - r, y, x + r, y, 0.8);
    pen.line_w(x, y - r, x, y + r, 0.8);
}

fn screen(pen: &Pen, x0: f32, y0: f32, x1: f32, y1: f32) {
    let r = pen.rect_screen(x0, y0, x1, y1);
    pen.painter.rect_filled(r, (0.2 * pen.s).clamp(1.0, 5.0), Color32::from_rgb(16, 24, 26));
}

pub fn draw_symbol(pen: &Pen, kind: Kind, params: &Params, live: &Live) {
    let p = pal();
    let ink = pen.stroke.color;
    let glow = live.glow.clamp(0.0, 1.0);
    match kind {
        Kind::Resistor => {
            pen.line(-3.0, 0.0, -2.0, 0.0);
            pen.poly(&[(-2.0, 0.0), (-1.67, -0.6), (-1.0, 0.6), (-0.33, -0.6), (0.33, 0.6), (1.0, -0.6), (1.67, 0.6), (2.0, 0.0)], false);
            pen.line(2.0, 0.0, 3.0, 0.0);
        }
        Kind::Potentiometer => {
            pen.line(-3.0, 0.0, -2.0, 0.0);
            pen.poly(&[(-2.0, 0.0), (-1.67, -0.6), (-1.0, 0.6), (-0.33, -0.6), (0.33, 0.6), (1.0, -0.6), (1.67, 0.6), (2.0, 0.0)], false);
            pen.line(2.0, 0.0, 3.0, 0.0);
            let wx = -2.0 + 4.0 * (params.setting as f32 / 100.0).clamp(0.0, 1.0);
            pen.poly(&[(0.0, -3.0), (0.0, -2.0), (wx, -2.0), (wx, -1.2)], false);
            arrow_head(pen, (wx, -0.7), (wx, -1.5), 0.6);
        }
        Kind::Capacitor => {
            pen.line(-3.0, 0.0, -0.35, 0.0);
            pen.line(0.35, 0.0, 3.0, 0.0);
            pen.line_w(-0.35, -1.1, -0.35, 1.1, 1.3);
            pen.line_w(0.35, -1.1, 0.35, 1.1, 1.3);
        }
        Kind::Inductor => {
            pen.line(-3.0, 0.0, -2.0, 0.0);
            let mut pts = Vec::new();
            for h in 0..4 {
                let cx = -1.5 + h as f32;
                for i in 0..=10 {
                    let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                    pts.push((cx + 0.5 * a.cos(), 0.6 * a.sin()));
                }
            }
            pen.poly(&pts, false);
            pen.line(2.0, 0.0, 3.0, 0.0);
        }
        Kind::DcVoltage => {
            pen.line(0.0, -3.0, 0.0, -0.4);
            pen.line_w(-1.3, -0.4, 1.3, -0.4, 1.0);
            pen.line_w(-0.6, 0.4, 0.6, 0.4, 2.2);
            pen.line(0.0, 0.4, 0.0, 3.0);
            plus(pen, 1.0, -1.3, 0.35);
        }
        Kind::DcCurrent | Kind::AcCurrent => {
            pen.line(0.0, -3.0, 0.0, -1.4);
            pen.circle(0.0, 0.0, 1.4);
            pen.line(0.0, 0.9, 0.0, -0.3);
            arrow_head(pen, (0.0, -0.9), (0.0, 0.0), 0.6);
            if kind == Kind::AcCurrent {
                sine_glyph(pen, 0.75, 0.55, 0.7, 0.4);
            }
            pen.line(0.0, 1.4, 0.0, 3.0);
        }
        Kind::AcVoltage | Kind::ClockSource => {
            pen.line(0.0, -3.0, 0.0, -1.4);
            pen.circle(0.0, 0.0, 1.4);
            if kind == Kind::AcVoltage {
                sine_glyph(pen, 0.0, 0.0, 1.8, 1.0);
            } else {
                square_glyph(pen, 0.0, 0.0, 1.6, 0.9);
            }
            pen.line(0.0, 1.4, 0.0, 3.0);
            plus(pen, 1.3, -1.9, 0.3);
        }
        Kind::Ground => {
            pen.line(0.0, 0.0, 0.0, 0.8);
            pen.line_w(-1.4, 0.8, 1.4, 0.8, 1.2);
            pen.line_w(-0.9, 1.35, 0.9, 1.35, 1.2);
            pen.line_w(-0.4, 1.9, 0.4, 1.9, 1.2);
        }
        Kind::Diode | Kind::Zener | Kind::Led => {
            pen.line(-3.0, 0.0, -0.8, 0.0);
            pen.line(0.8, 0.0, 3.0, 0.0);
            let lit = kind == Kind::Led && glow > 0.02;
            let [r, g, b] = params.led.rgb();
            let led_col = Color32::from_rgb(r, g, b);
            if lit {
                // soft halo, several rings for a smooth falloff
                for k in 0..5 {
                    let f = 1.0 - k as f32 / 5.0;
                    let a = (glow * 70.0 * f) as u8;
                    pen.fill_circle(0.0, 0.0, 0.9 + (0.6 + 2.2 * glow) * (k as f32 + 1.0) / 5.0, Color32::from_rgba_unmultiplied(r, g, b, a));
                }
            }
            let fill = if kind == Kind::Led { blend(if p.dark { p.box_fill } else { Color32::WHITE }, led_col, if lit { 0.4 + 0.6 * glow } else { 0.25 }) } else { ink };
            pen.fill_poly(&[(-0.8, -0.9), (-0.8, 0.9), (0.8, 0.0)], fill);
            if kind == Kind::Zener {
                pen.poly(&[(0.5, -1.1), (0.8, -0.9), (0.8, 0.9), (1.1, 1.1)], false);
            } else {
                pen.line(0.8, -0.9, 0.8, 0.9);
            }
            if kind == Kind::Led {
                pen.line(0.0, -1.1, 0.7, -1.8);
                arrow_head(pen, (0.9, -2.0), (0.0, -1.1), 0.45);
                pen.line(0.7, -0.9, 1.4, -1.6);
                arrow_head(pen, (1.6, -1.8), (0.7, -0.9), 0.45);
            }
        }
        Kind::Switch => {
            pen.line(-3.0, 0.0, -1.2, 0.0);
            pen.line(1.2, 0.0, 3.0, 0.0);
            pen.fill_circle(-1.2, 0.0, 0.2, ink);
            pen.fill_circle(1.2, 0.0, 0.2, ink);
            if params.closed {
                pen.line(-1.2, 0.0, 1.2, -0.15);
            } else {
                pen.line(-1.2, 0.0, 0.9, -1.2);
            }
        }
        Kind::PushButton => {
            pen.line(-3.0, 0.0, -1.2, 0.0);
            pen.line(1.2, 0.0, 3.0, 0.0);
            pen.fill_circle(-1.2, 0.0, 0.2, ink);
            pen.fill_circle(1.2, 0.0, 0.2, ink);
            let on = params.closed != params.normally_closed;
            let y = if on { -0.25 } else { -0.8 };
            pen.line_w(-1.4, y, 1.4, y, 1.3);
            pen.line(0.0, y, 0.0, -1.6);
            pen.line_w(-0.6, -1.6, 0.6, -1.6, 1.3);
        }
        Kind::SpdtSwitch => {
            pen.line(-3.0, 0.0, -1.0, 0.0);
            pen.fill_circle(-1.0, 0.0, 0.2, ink);
            pen.poly(&[(3.0, -2.0), (1.6, -2.0), (1.2, -1.0)], false);
            pen.poly(&[(3.0, 2.0), (1.6, 2.0), (1.2, 1.0)], false);
            pen.fill_circle(1.2, -1.0, 0.2, ink);
            pen.fill_circle(1.2, 1.0, 0.2, ink);
            if params.closed {
                pen.line(-1.0, 0.0, 1.1, 0.95);
            } else {
                pen.line(-1.0, 0.0, 1.1, -0.95);
            }
        }
        Kind::Fuse => {
            pen.line(-3.0, 0.0, -1.6, 0.0);
            pen.line(1.6, 0.0, 3.0, 0.0);
            let r = pen.rect_screen(-1.6, -0.7, 1.6, 0.7);
            pen.painter.rect(r, 0.35 * pen.s, if params.blown { Color32::from_rgba_unmultiplied(220, 60, 40, 60) } else { Color32::TRANSPARENT }, pen.stroke);
            if params.blown {
                pen.line_c(-1.6, 0.0, -0.4, 0.0, ink);
                pen.line_c(0.4, 0.0, 1.6, 0.0, ink);
                pen.line_c(-0.4, 0.0, -0.1, -0.35, Color32::from_rgb(230, 70, 40));
                pen.line_c(0.4, 0.0, 0.1, 0.35, Color32::from_rgb(230, 70, 40));
            } else {
                let mut pts = Vec::new();
                for i in 0..=20 {
                    let t = i as f32 / 20.0;
                    pts.push((-1.6 + 3.2 * t, -0.35 * (t * std::f32::consts::TAU).sin()));
                }
                pen.poly(&pts, false);
            }
        }
        Kind::Lamp => {
            if glow > 0.02 {
                for k in 0..5 {
                    let f = 1.0 - k as f32 / 5.0;
                    let a = (glow * 90.0 * f) as u8;
                    pen.fill_circle(0.0, 0.0, 1.3 + 1.6 * glow * (k as f32 + 1.0) / 5.0, Color32::from_rgba_unmultiplied(255, 210, 60, a));
                }
                pen.fill_circle(0.0, 0.0, 1.25, Color32::from_rgba_unmultiplied(255, 236, 140, (glow * 255.0) as u8));
            }
            pen.line(-3.0, 0.0, -1.3, 0.0);
            pen.line(1.3, 0.0, 3.0, 0.0);
            pen.circle(0.0, 0.0, 1.3);
            pen.line(-0.92, -0.92, 0.92, 0.92);
            pen.line(-0.92, 0.92, 0.92, -0.92);
        }
        Kind::Motor => {
            pen.line(-3.0, 0.0, -1.5, 0.0);
            pen.line(1.5, 0.0, 3.0, 0.0);
            pen.fill_circle(0.0, 0.0, 1.5, p.box_fill);
            pen.circle(0.0, 0.0, 1.5);
            // spinning rotor blades
            for k in 0..3 {
                let a = live.spin + k as f32 * std::f32::consts::TAU / 3.0;
                let (sx, sy) = (a.cos() * 1.05, a.sin() * 1.05);
                pen.line_c(0.0, 0.0, sx, sy, blend(ink, p.accent, 0.5));
            }
            pen.fill_circle(0.0, 0.0, 0.25, ink);
        }
        Kind::Buzzer => {
            pen.line(-3.0, 0.0, -1.0, 0.0);
            pen.line(1.0, 0.0, 3.0, 0.0);
            pen.line(-1.0, 0.0, -1.0, -0.6);
            pen.line(1.0, 0.0, 1.0, -0.6);
            let mut pts = Vec::new();
            for i in 0..=16 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 16.0);
                pts.push((1.4 * a.cos(), -0.6 + 1.4 * a.sin()));
            }
            pen.fill_poly(&pts, p.box_fill);
            // sound waves when active
            if glow > 0.05 {
                for k in 0..3 {
                    let ph = (live.time * 3.0 + k as f32 / 3.0).fract();
                    let rr = 1.6 + ph * 1.2;
                    let a = ((1.0 - ph) * 200.0 * glow) as u8;
                    let col = Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), a);
                    let mut arc = Vec::new();
                    for i in 0..=8 {
                        let t = -0.9 + 1.8 * i as f32 / 8.0;
                        arc.push(pen.p(rr * t.sin(), -0.6 - rr * t.cos()));
                    }
                    pen.painter.add(Shape::line(arc, Stroke::new(pen.stroke.width, col)));
                }
            }
        }
        Kind::OpAmp => {
            pen.line(-4.0, -2.0, -3.0, -2.0);
            pen.line(-4.0, 2.0, -3.0, 2.0);
            pen.line(3.0, 0.0, 4.0, 0.0);
            pen.fill_poly(&[(-3.0, -3.0), (-3.0, 3.0), (3.0, 0.0)], p.box_fill);
            pen.line_w(-2.6, -2.0, -1.8, -2.0, 0.9);
            plus(pen, -2.2, 2.0, 0.4);
        }
        Kind::Voltmeter | Kind::Ammeter => {
            pen.line(-4.0, 0.0, -3.0, 0.0);
            pen.line(3.0, 0.0, 4.0, 0.0);
            pen.panel(-3.0, -1.8, 3.0, 1.8, p.box_fill);
            let lcd = pen.rect_screen(-2.5, -1.2, 2.5, 0.9);
            pen.painter.rect_filled(lcd, 0.2 * pen.s, p.lcd_bg);
            let txt = live.reading.clone().unwrap_or_else(|| if kind == Kind::Voltmeter { "---- V".into() } else { "---- A".into() });
            pen.text_fixed(lcd.center(), &txt, 0.9, p.lcd_text, Align2::CENTER_CENTER);
            plus(pen, -2.4, 1.35, 0.25);
            pen.line_w(2.1, 1.35, 2.6, 1.35, 0.8);
        }
        Kind::Probe => {
            pen.line(0.0, 0.0, 1.0, -1.2);
            pen.fill_circle(0.0, 0.0, 0.25, ink);
            let txt = live.reading.clone().unwrap_or_else(|| "V: ---".into());
            let fs = (0.85 * pen.s).clamp(5.0, 40.0);
            let galley = pen.painter.layout_no_wrap(txt, FontId::proportional(fs), p.lcd_text);
            let tl = pen.origin + vec2(1.0 * pen.s, -3.2 * pen.s);
            let r = Rect::from_min_size(tl, vec2((galley.size().x + 8.0).max(4.6 * pen.s), 2.0 * pen.s));
            pen.painter.rect_filled(r.translate(vec2(0.0, 2.0)), 4.0, Color32::from_black_alpha(40));
            pen.painter.rect(r, 4.0, p.lcd_bg, Stroke::new(1.0, p.accent));
            let gpos = pos2(r.left() + 4.0, r.center().y - galley.size().y / 2.0);
            pen.painter.galley(gpos, galley, p.lcd_text);
        }
        Kind::Multimeter => {
            pen.line(-1.0, 3.0, -1.0, 4.0);
            pen.line(1.0, 3.0, 1.0, 4.0);
            pen.panel(-2.0, -3.0, 2.0, 3.0, p.box_fill);
            let lcd = pen.rect_screen(-1.5, -2.4, 1.5, -0.8);
            pen.painter.rect_filled(lcd, 0.2 * pen.s, p.lcd_bg);
            pen.text_fixed(lcd.center(), "8.88", 0.7, p.lcd_text, Align2::CENTER_CENTER);
            pen.fill_circle(0.0, 0.8, 0.7, p.box_fill2);
            pen.circle(0.0, 0.8, 0.7);
            pen.line(0.0, 0.8, 0.4, 0.3);
            pen.text(-1.0, 2.3, "+", 0.9, ink, Align2::CENTER_CENTER);
            pen.text(1.0, 2.3, "−", 0.9, ink, Align2::CENTER_CENTER);
        }
        Kind::FuncGen => {
            for x in [-2.0, 0.0, 2.0] {
                pen.line(x, 3.0, x, 4.0);
            }
            pen.panel(-3.0, -3.0, 3.0, 3.0, p.box_fill);
            screen(pen, -2.4, -2.4, 2.4, 0.6);
            let green = Pen { stroke: Stroke::new(pen.stroke.width, Color32::from_rgb(90, 255, 140)), ..*pen };
            sine_glyph(&green, 0.0, -0.9, 4.0, 2.0);
            pen.text(-2.0, 2.0, "+", 0.8, ink, Align2::CENTER_CENTER);
            pen.text(0.0, 2.0, "C", 0.75, ink, Align2::CENTER_CENTER);
            pen.text(2.0, 2.0, "−", 0.8, ink, Align2::CENTER_CENTER);
        }
        Kind::Oscilloscope => {
            for x in [-3.0, -1.0, 1.0, 3.0] {
                pen.line(x, 3.0, x, 4.0);
            }
            pen.panel(-4.0, -3.0, 4.0, 3.0, p.box_fill);
            screen(pen, -3.3, -2.5, 3.3, 1.2);
            let green = Pen { stroke: Stroke::new(pen.stroke.width, Color32::from_rgb(80, 255, 120)), ..*pen };
            sine_glyph(&green, 0.0, -0.65, 6.0, 2.2);
            for (x, t) in [(-3.0, "A"), (-1.0, "B"), (1.0, "T"), (3.0, "G")] {
                pen.text(x, 2.2, t, 0.75, ink, Align2::CENTER_CENTER);
            }
        }
        Kind::Wattmeter => {
            for x in [-3.0, -1.0, 1.0, 3.0] {
                pen.line(x, 3.0, x, 4.0);
            }
            pen.panel(-4.0, -3.0, 4.0, 3.0, p.box_fill);
            let lcd = pen.rect_screen(-3.3, -2.5, 3.3, 0.8);
            pen.painter.rect_filled(lcd, 0.2 * pen.s, p.lcd_bg);
            let txt = live.reading.clone().unwrap_or_else(|| "---- W".into());
            pen.text_fixed(lcd.center(), &txt, 0.85, p.lcd_text, Align2::CENTER_CENTER);
            for (x, t) in [(-3.0, "V+"), (-1.0, "V−"), (1.0, "I+"), (3.0, "I−")] {
                pen.text(x, 2.1, t, 0.65, ink, Align2::CENTER_CENTER);
            }
        }
        Kind::BodePlotter => {
            pen.line(-2.0, 3.0, -2.0, 4.0);
            pen.line(2.0, 3.0, 2.0, 4.0);
            pen.panel(-3.0, -3.0, 3.0, 3.0, p.box_fill);
            screen(pen, -2.4, -2.4, 2.4, 0.8);
            let green = Pen { stroke: Stroke::new(pen.stroke.width, Color32::from_rgb(90, 220, 255)), ..*pen };
            green.poly(&[(-2.1, -1.6), (-0.3, -1.6), (0.4, -1.2), (1.0, -0.4), (2.1, 0.5)], false);
            pen.text(-2.0, 2.1, "IN", 0.6, ink, Align2::CENTER_CENTER);
            pen.text(2.0, 2.1, "OUT", 0.6, ink, Align2::CENTER_CENTER);
        }
        Kind::FreqCounter => {
            pen.line(0.0, 3.0, 0.0, 4.0);
            pen.panel(-2.5, -3.0, 2.5, 3.0, p.box_fill);
            let lcd = pen.rect_screen(-2.0, -2.4, 2.0, 0.2);
            pen.painter.rect_filled(lcd, 0.2 * pen.s, p.lcd_bg);
            let txt = live.reading.clone().unwrap_or_else(|| "--- Hz".into());
            pen.text_fixed(lcd.center(), &txt, 0.7, p.lcd_text, Align2::CENTER_CENTER);
            pen.text(0.0, 1.6, "Hz", 0.75, ink, Align2::CENTER_CENTER);
        }
    }
}

/// Draw the reference and value labels around a placed component.
pub fn draw_labels(painter: &Painter, comp: &Component, origin: Pos2, s: f32) {
    let p = pal();
    if comp.kind == Kind::Ground || comp.kind == Kind::Probe {
        if comp.kind == Kind::Probe {
            painter.text(origin + vec2(1.2 * s, -3.4 * s), Align2::LEFT_BOTTOM, &comp.refdes, FontId::proportional((0.85 * s).clamp(5.0, 30.0)), p.label);
        }
        return;
    }
    let (x0, y0, x1, y1) = comp.kind.bounds();
    let mut r = Rect::NOTHING;
    for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
        let (rx, ry) = rotate_f(x, y, comp.rot);
        r.extend_with(origin + vec2(rx * s, ry * s));
    }
    let fs = (1.0 * s).clamp(5.0, 32.0);
    let val = comp.value_label();
    let vertical_src = matches!(comp.kind, Kind::DcVoltage | Kind::DcCurrent | Kind::AcVoltage | Kind::ClockSource | Kind::AcCurrent);
    let vertical = (comp.rot % 2 == 1) != vertical_src;
    if comp.kind.is_instrument() {
        painter.text(pos2(r.center().x, r.top() - 2.0), Align2::CENTER_BOTTOM, &comp.refdes, FontId::proportional(fs), p.label);
    } else if vertical {
        let x = r.right() + 0.4 * s;
        painter.text(pos2(x, r.center().y - 0.2 * s), Align2::LEFT_BOTTOM, &comp.refdes, FontId::proportional(fs), p.label);
        painter.text(pos2(x, r.center().y + 0.2 * s), Align2::LEFT_TOP, &val, FontId::proportional(fs), p.value);
    } else {
        painter.text(pos2(r.center().x, r.top() - 0.2 * s), Align2::CENTER_BOTTOM, &comp.refdes, FontId::proportional(fs), p.label);
        if !val.is_empty() {
            painter.text(pos2(r.center().x, r.bottom() + 0.2 * s), Align2::CENTER_TOP, &val, FontId::proportional(fs), p.value);
        }
    }
}

/// Small icon of a part for the parts bin.
pub fn draw_icon(painter: &Painter, rect: Rect, kind: Kind, ink: Color32) {
    let (x0, y0, x1, y1) = kind.bounds();
    let w = (x1 - x0).max(1.0);
    let h = (y1 - y0).max(1.0);
    let s = (rect.width() / w).min(rect.height() / h) * 0.9;
    let cx = (x0 + x1) / 2.0;
    let cy = (y0 + y1) / 2.0;
    let origin = rect.center() - vec2(cx * s, cy * s);
    let pen = Pen { painter, origin, s, rot: 0, stroke: Stroke::new(1.2, ink) };
    let mut p = kind.default_params();
    if kind == Kind::Switch {
        p.closed = false;
    }
    let live = Live { glow: 0.0, reading: Some(String::new()), ..Default::default() };
    draw_symbol(&pen, kind, &p, &live);
}
