//! Virtual instruments: live measurement state and their front panels.

use crate::model::*;
use crate::theme::pal;
use egui::{pos2, vec2, Align2, Color32, FontId, RichText, Rect, Sense, Shape, Stroke, Ui};
use std::collections::VecDeque;

// ---------------------------------------------------------------- meter averaging

/// Rolling window of samples used for DC average / AC RMS readings.
#[derive(Clone, Default)]
pub struct Meter {
    pub samples: VecDeque<(f64, f64)>,
    pub window: f64,
}

impl Meter {
    pub fn push(&mut self, t: f64, v: f64) {
        self.samples.push_back((t, v));
        let cutoff = t - self.window;
        while self.samples.len() > 1 && self.samples.front().map(|s| s.0 < cutoff).unwrap_or(false) {
            self.samples.pop_front();
        }
        while self.samples.len() > 50_000 {
            self.samples.pop_front();
        }
    }
    /// Time-weighted average over roughly the last `dt` seconds (for smooth visual effects).
    pub fn mean_recent(&self, dt: f64) -> f64 {
        let Some(&(t_end, _)) = self.samples.back() else {
            return 0.0;
        };
        let mut acc = 0.0;
        let mut tt = 0.0;
        let mut prev: Option<(f64, f64)> = None;
        for &(t, v) in self.samples.iter().rev() {
            if let Some((pt, pv)) = prev {
                let d = pt - t;
                acc += 0.5 * (v + pv) * d;
                tt += d;
            }
            prev = Some((t, v));
            if t_end - t >= dt {
                break;
            }
        }
        if tt > 0.0 {
            acc / tt
        } else {
            self.last()
        }
    }

    pub fn last(&self) -> f64 {
        self.samples.back().map(|s| s.1).unwrap_or(0.0)
    }
    pub fn mean(&self) -> f64 {
        if self.samples.len() < 2 {
            return self.last();
        }
        // time-weighted average
        let mut acc = 0.0;
        let mut tt = 0.0;
        for w in self.samples.iter().collect::<Vec<_>>().windows(2) {
            let dt = w[1].0 - w[0].0;
            acc += 0.5 * (w[0].1 + w[1].1) * dt;
            tt += dt;
        }
        if tt > 0.0 {
            acc / tt
        } else {
            self.last()
        }
    }
    /// RMS of the AC part (DC removed).
    pub fn ac_rms(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let m = self.mean();
        let mut acc = 0.0;
        let mut tt = 0.0;
        let v: Vec<_> = self.samples.iter().collect();
        for w in v.windows(2) {
            let dt = w[1].0 - w[0].0;
            let a = w[0].1 - m;
            let b = w[1].1 - m;
            acc += 0.5 * (a * a + b * b) * dt;
            tt += dt;
        }
        if tt > 0.0 {
            (acc / tt).sqrt()
        } else {
            0.0
        }
    }
    pub fn peak_to_peak(&self) -> f64 {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for s in &self.samples {
            lo = lo.min(s.1);
            hi = hi.max(s.1);
        }
        if hi >= lo {
            hi - lo
        } else {
            0.0
        }
    }
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

// ---------------------------------------------------------------- oscilloscope

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrigMode {
    Auto,
    Normal,
    Single,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrigSrc {
    A,
    B,
    Ext,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Coupling {
    Ac,
    Zero,
    Dc,
}

#[derive(Clone)]
pub struct Channel {
    pub scale: f64,
    pub pos: f64,
    pub coupling: Coupling,
    pub on: bool,
}

#[derive(Clone)]
pub struct Scope {
    /// (t, a, b, ext)
    pub buf: VecDeque<[f64; 4]>,
    pub timebase: f64,
    pub xpos: f64,
    pub ch: [Channel; 2],
    pub trig_level: f64,
    pub trig_rising: bool,
    pub trig_src: TrigSrc,
    pub mode: TrigMode,
    pub display: Vec<[f64; 4]>,
    pub disp_t0: f64,
    pub single_done: bool,
    pub cursor: [f32; 2],
    pub reverse: bool,
    pub last_store_t: f64,
    pub dc_a: f64,
    pub dc_b: f64,
}

impl Default for Scope {
    fn default() -> Self {
        Scope {
            buf: VecDeque::new(),
            timebase: 200e-6,
            xpos: 0.0,
            ch: [
                Channel { scale: 2.0, pos: 0.0, coupling: Coupling::Dc, on: true },
                Channel { scale: 2.0, pos: 0.0, coupling: Coupling::Dc, on: true },
            ],
            trig_level: 0.0,
            trig_rising: true,
            trig_src: TrigSrc::A,
            mode: TrigMode::Auto,
            display: Vec::new(),
            disp_t0: 0.0,
            single_done: false,
            cursor: [0.25, 0.6],
            reverse: false,
            last_store_t: f64::NEG_INFINITY,
            dc_a: 0.0,
            dc_b: 0.0,
        }
    }
}

pub fn step_125(v: f64, up: bool) -> f64 {
    let e = v.log10().floor();
    let base = 10f64.powf(e);
    let m = (v / base * 1000.0).round() / 1000.0;
    let seq = [1.0, 2.0, 5.0, 10.0];
    if up {
        for s in seq {
            if s > m + 1e-9 {
                return s * base;
            }
        }
        20.0 * base
    } else {
        for s in seq.iter().rev() {
            let s = if *s == 10.0 { 10.0 } else { *s };
            if s < m - 1e-9 {
                return s * base;
            }
        }
        0.5 * base
    }
}

impl Scope {
    pub fn push(&mut self, t: f64, a: f64, b: f64, ext: f64) {
        let min_dt = self.timebase * 10.0 / 4000.0;
        if t - self.last_store_t >= min_dt {
            self.buf.push_back([t, a, b, ext]);
            self.last_store_t = t;
        }
        let keep = self.timebase * 40.0;
        while self.buf.front().map(|s| s[0] < t - keep).unwrap_or(false) && self.buf.len() > 2 {
            self.buf.pop_front();
        }
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.display.clear();
        self.single_done = false;
        self.last_store_t = f64::NEG_INFINITY;
    }

    /// Choose what to show on screen, honouring the trigger settings.
    pub fn update_display(&mut self) {
        let Some(last) = self.buf.back() else {
            return;
        };
        let now = last[0];
        let span = self.timebase * 10.0;
        // average levels for AC coupling
        if !self.buf.is_empty() {
            let n = self.buf.len() as f64;
            self.dc_a = self.buf.iter().map(|s| s[1]).sum::<f64>() / n;
            self.dc_b = self.buf.iter().map(|s| s[2]).sum::<f64>() / n;
        }
        if self.mode == TrigMode::Single && self.single_done {
            return;
        }
        let idx = match self.trig_src {
            TrigSrc::A => 1,
            TrigSrc::B => 2,
            TrigSrc::Ext => 3,
        };
        let pre = self.xpos.max(0.0) * self.timebase;
        let mut found: Option<f64> = None;
        let v: Vec<[f64; 4]> = self.buf.iter().copied().collect();
        for k in (1..v.len()).rev() {
            let (p, q) = (v[k - 1], v[k]);
            if q[0] + span - pre > now {
                continue;
            }
            let lvl = self.trig_level;
            let hit = if self.trig_rising { p[idx] < lvl && q[idx] >= lvl } else { p[idx] > lvl && q[idx] <= lvl };
            if hit {
                let f = if (q[idx] - p[idx]).abs() > 1e-15 { (lvl - p[idx]) / (q[idx] - p[idx]) } else { 0.0 };
                found = Some(p[0] + f * (q[0] - p[0]));
                break;
            }
        }
        let t0 = match (found, self.mode) {
            (Some(tc), _) => tc - pre,
            (None, TrigMode::Auto) => now - span,
            (None, _) => return,
        };
        self.disp_t0 = t0;
        self.display = v.into_iter().filter(|s| s[0] >= t0 && s[0] <= t0 + span).collect();
        if self.mode == TrigMode::Single && found.is_some() {
            self.single_done = true;
        }
    }

    fn value_at(&self, ch: usize, t: f64) -> Option<f64> {
        let d = &self.display;
        if d.is_empty() {
            return None;
        }
        let i = d.partition_point(|s| s[0] < t);
        let s = if i >= d.len() { d[d.len() - 1] } else { d[i] };
        Some(s[ch + 1])
    }

    fn shown(&self, ch: usize, v: f64) -> f64 {
        match self.ch[ch].coupling {
            Coupling::Dc => v,
            Coupling::Zero => 0.0,
            Coupling::Ac => v - if ch == 0 { self.dc_a } else { self.dc_b },
        }
    }
}

fn scale_row(ui: &mut Ui, label: &str, value: &mut f64, unit: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add_sized([86.0, 20.0], egui::Label::new(RichText::new(format!("{}/Div", fmt_si(*value, unit))).monospace()));
        if ui.small_button(" − ").on_hover_text("Smaller per division").clicked() {
            *value = step_125(*value, false);
        }
        if ui.small_button(" + ").on_hover_text("Larger per division").clicked() {
            *value = step_125(*value, true);
        }
    });
}

/// Draw the oscilloscope front panel. `colors` are the trace colours for A and B.
pub fn scope_panel(ui: &mut Ui, sc: &mut Scope, colors: [Color32; 2]) {
    let w = 520.0;
    let h = 220.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    let bg = if sc.reverse { Color32::WHITE } else { Color32::BLACK };
    let grid = if sc.reverse { Color32::from_gray(190) } else { Color32::from_rgb(40, 70, 50) };
    p.rect_filled(rect, 3.0, bg);
    for i in 0..=10 {
        let x = rect.left() + rect.width() * i as f32 / 10.0;
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(if i == 5 { 1.2 } else { 0.6 }, grid));
    }
    for j in 0..=8 {
        let y = rect.top() + rect.height() * j as f32 / 8.0;
        p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(if j == 4 { 1.2 } else { 0.6 }, grid));
    }
    let span = sc.timebase * 10.0;
    for ch in 0..2 {
        if !sc.ch[ch].on {
            continue;
        }
        let mut pts = Vec::with_capacity(sc.display.len());
        for s in &sc.display {
            let fx = ((s[0] - sc.disp_t0) / span) as f32;
            let v = sc.shown(ch, s[ch + 1]);
            let fy = (v / sc.ch[ch].scale + sc.ch[ch].pos) as f32; // in divisions from centre
            let y = rect.center().y - fy * rect.height() / 8.0;
            pts.push(pos2(rect.left() + fx * rect.width(), y.clamp(rect.top() - 2.0, rect.bottom() + 2.0)));
        }
        if pts.len() > 1 {
            // phosphor glow under a crisp trace
            let c = colors[ch];
            p.add(Shape::line(pts.clone(), Stroke::new(6.0, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 40))));
            p.add(Shape::line(pts.clone(), Stroke::new(3.0, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 70))));
            p.add(Shape::line(pts, Stroke::new(1.5, c)));
        }
        // channel ground marker
        let gy = rect.center().y - sc.ch[ch].pos as f32 * rect.height() / 8.0;
        p.text(pos2(rect.left() + 2.0, gy), Align2::LEFT_CENTER, if ch == 0 { "A>" } else { "B>" }, FontId::monospace(10.0), colors[ch]);
    }
    // trigger level marker
    let tch = if sc.trig_src == TrigSrc::B { 1 } else { 0 };
    let ty = rect.center().y - ((sc.trig_level / sc.ch[tch].scale + sc.ch[tch].pos) as f32) * rect.height() / 8.0;
    if ty > rect.top() && ty < rect.bottom() {
        p.text(pos2(rect.right() - 2.0, ty), Align2::RIGHT_CENTER, "<T", FontId::monospace(10.0), Color32::from_rgb(230, 200, 60));
    }
    // cursors
    let ccol = [Color32::from_rgb(255, 80, 80), Color32::from_rgb(90, 150, 255)];
    for k in 0..2 {
        let x = rect.left() + sc.cursor[k] * rect.width();
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, ccol[k]));
        p.text(pos2(x + 2.0, rect.top() + 2.0), Align2::LEFT_TOP, if k == 0 { "1" } else { "2" }, FontId::monospace(11.0), ccol[k]);
    }
    if resp.dragged() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let f = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let k = if (f - sc.cursor[0]).abs() <= (f - sc.cursor[1]).abs() { 0 } else { 1 };
            sc.cursor[k] = f;
        }
    }
    if sc.display.is_empty() {
        p.text(rect.center(), Align2::CENTER_CENTER, "Run the simulation to see waveforms", FontId::proportional(14.0), Color32::GRAY);
    }

    // cursor readout table
    ui.add_space(4.0);
    egui::Grid::new("scope_readout").striped(true).num_columns(4).show(ui, |ui| {
        ui.label("");
        ui.label(RichText::new("Time").strong());
        ui.label(RichText::new("Channel A").strong().color(colors[0]));
        ui.label(RichText::new("Channel B").strong().color(colors[1]));
        ui.end_row();
        let mut vals = [[0.0f64; 3]; 2];
        for k in 0..2 {
            let t = sc.disp_t0 + sc.cursor[k] as f64 * span;
            let a = sc.value_at(0, t).map(|v| sc.shown(0, v)).unwrap_or(0.0);
            let b = sc.value_at(1, t).map(|v| sc.shown(1, v)).unwrap_or(0.0);
            vals[k] = [t, a, b];
            ui.label(RichText::new(format!("T{}", k + 1)).color(ccol[k]));
            ui.monospace(fmt_reading(t, "s"));
            ui.monospace(fmt_reading(a, "V"));
            ui.monospace(fmt_reading(b, "V"));
            ui.end_row();
        }
        ui.label("T2−T1");
        ui.monospace(fmt_reading(vals[1][0] - vals[0][0], "s"));
        ui.monospace(fmt_reading(vals[1][1] - vals[0][1], "V"));
        ui.monospace(fmt_reading(vals[1][2] - vals[0][2], "V"));
        ui.end_row();
    });
    ui.separator();
    egui::Grid::new("scope_ctl").num_columns(2).spacing([6.0, 6.0]).show(ui, |ui| {
        ui.group(|ui| {
            ui.set_min_width(240.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Timebase").strong());
                let mut tb = sc.timebase;
                scale_row(ui, "Scale", &mut tb, "s");
                if tb != sc.timebase {
                    sc.timebase = tb;
                    sc.reset();
                }
                ui.horizontal(|ui| {
                    ui.label("X pos (Div)");
                    ui.add(egui::DragValue::new(&mut sc.xpos).speed(0.1).clamp_range(0.0..=10.0));
                    ui.checkbox(&mut sc.reverse, "White screen");
                });
            });
        });
        ui.group(|ui| {
            ui.set_min_width(240.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("Trigger").strong());
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut sc.trig_rising, true, "Rise");
                    ui.selectable_value(&mut sc.trig_rising, false, "Fall");
                    ui.label("  Source");
                    ui.selectable_value(&mut sc.trig_src, TrigSrc::A, "A");
                    ui.selectable_value(&mut sc.trig_src, TrigSrc::B, "B");
                    ui.selectable_value(&mut sc.trig_src, TrigSrc::Ext, "Ext");
                });
                ui.horizontal(|ui| {
                    ui.label("Level (V)");
                    ui.add(egui::DragValue::new(&mut sc.trig_level).speed(0.05));
                    if ui.selectable_label(sc.mode == TrigMode::Single, "Single").clicked() {
                        sc.mode = TrigMode::Single;
                        sc.single_done = false;
                    }
                    ui.selectable_value(&mut sc.mode, TrigMode::Normal, "Normal");
                    ui.selectable_value(&mut sc.mode, TrigMode::Auto, "Auto");
                });
            });
        });
        ui.end_row();
        for ch in 0..2 {
            ui.group(|ui| {
                ui.set_min_width(240.0);
                ui.vertical(|ui| {
                    ui.checkbox(&mut sc.ch[ch].on, RichText::new(if ch == 0 { "Channel A" } else { "Channel B" }).strong().color(colors[ch]));
                    scale_row(ui, "Scale", &mut sc.ch[ch].scale, "V");
                    ui.horizontal(|ui| {
                        ui.label("Y pos (Div)");
                        ui.add(egui::DragValue::new(&mut sc.ch[ch].pos).speed(0.1).clamp_range(-4.0..=4.0));
                        ui.selectable_value(&mut sc.ch[ch].coupling, Coupling::Ac, "AC");
                        ui.selectable_value(&mut sc.ch[ch].coupling, Coupling::Zero, "0");
                        ui.selectable_value(&mut sc.ch[ch].coupling, Coupling::Dc, "DC");
                    });
                });
            });
        }
        ui.end_row();
    });
}

// ---------------------------------------------------------------- multimeter

pub fn multimeter_panel(ui: &mut Ui, params: &mut Params, reading: &str) -> bool {
    let mut changed = false;
    let (rect, _) = ui.allocate_exact_size(vec2(260.0, 64.0), Sense::hover());
    let p = ui.painter_at(rect);
    let pl = pal();
    p.rect(rect, 8.0, pl.lcd_bg, Stroke::new(2.0, pl.accent));
    p.text(rect.center(), Align2::CENTER_CENTER, reading, FontId::monospace(28.0), pl.lcd_text);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        for (m, t) in [(MeterMode::Amps, "A"), (MeterMode::Volts, "V"), (MeterMode::Ohms, "Ω"), (MeterMode::Decibel, "dB")] {
            let b = egui::Button::new(RichText::new(t).size(18.0)).selected(params.meter == m).min_size(vec2(56.0, 30.0));
            if ui.add(b).clicked() && params.meter != m {
                params.meter = m;
                changed = true;
            }
        }
    });
    ui.horizontal(|ui| {
        let b = egui::Button::new(RichText::new("AC  ~").size(16.0)).selected(params.ac).min_size(vec2(118.0, 26.0));
        if ui.add(b).clicked() {
            params.ac = true;
        }
        let b = egui::Button::new(RichText::new("DC  =").size(16.0)).selected(!params.ac).min_size(vec2(118.0, 26.0));
        if ui.add(b).clicked() {
            params.ac = false;
        }
    });
    ui.label(
        RichText::new(match params.meter {
            MeterMode::Volts => "Connect + and − across the part to measure voltage.",
            MeterMode::Amps => "Connect in series (break the wire) to measure current.",
            MeterMode::Ohms => "Measure a part with the power removed.",
            MeterMode::Decibel => "dB relative to 0.775 V (1 mW into 600 Ω).",
        })
        .small()
        .weak(),
    );
    changed
}

// ---------------------------------------------------------------- function generator

/// A text field holding an SI value: edits apply when Enter is pressed or focus leaves.
pub fn si_field(ui: &mut Ui, id: egui::Id, value: &mut f64, unit: &str, width: f32) -> bool {
    let mut text: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| fmt_si(*value, unit));
    let resp = ui.add(egui::TextEdit::singleline(&mut text).id(id.with("te")).desired_width(width));
    let mut changed = false;
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    } else {
        if resp.lost_focus() {
            if let Some(v) = parse_si(&text) {
                if v != *value {
                    *value = v;
                    changed = true;
                }
            }
        }
        ui.data_mut(|d| d.remove::<String>(id));
    }
    changed
}

pub fn funcgen_panel(ui: &mut Ui, id: egui::Id, params: &mut Params) -> bool {
    let mut changed = false;
    ui.label(RichText::new("Waveforms").strong());
    ui.horizontal(|ui| {
        for (w, t) in [(Wave::Sine, "Sine"), (Wave::Triangle, "Triangle"), (Wave::Square, "Square")] {
            let b = egui::Button::new(RichText::new(t).size(15.0)).selected(params.wave == w).min_size(vec2(84.0, 28.0));
            if ui.add(b).clicked() {
                params.wave = w;
                changed = true;
            }
        }
    });
    ui.add_space(4.0);
    ui.label(RichText::new("Signal options").strong());
    egui::Grid::new(id.with("grid")).num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Frequency");
        changed |= si_field(ui, id.with("f"), &mut params.freq, "Hz", 110.0);
        ui.end_row();
        ui.label("Duty cycle");
        ui.add_enabled_ui(params.wave == Wave::Square, |ui| {
            changed |= ui.add(egui::DragValue::new(&mut params.duty).suffix(" %").clamp_range(1.0..=99.0).speed(0.5)).changed();
        });
        ui.end_row();
        ui.label("Amplitude");
        changed |= si_field(ui, id.with("a"), &mut params.value, "Vp", 110.0);
        ui.end_row();
        ui.label("Offset");
        changed |= si_field(ui, id.with("o"), &mut params.offset, "V", 110.0);
        ui.end_row();
    });
    ui.label(RichText::new("+ gives the signal, − gives it inverted, both relative to COM.").small().weak());
    changed
}

/// Small preview of a function generator's output.
pub fn wave_preview(ui: &mut Ui, params: &Params) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().min(280.0), 50.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_gray(25));
    let src = crate::sim::Src::Fg { wave: params.wave, amp: 1.0, freq: 2.0, offset: 0.0, duty: params.duty, invert: false };
    let pts: Vec<_> = (0..=200)
        .map(|i| {
            let t = i as f64 / 200.0;
            let v = src.at(t);
            pos2(rect.left() + rect.width() * t as f32, rect.center().y - v as f32 * rect.height() * 0.4)
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(1.5, Color32::from_rgb(90, 255, 140))));
    let _ = Rect::NOTHING;
}

// ---------------------------------------------------------------- LCD helper, wattmeter, frequency counter

/// A themed LCD readout with a big main line and an optional caption.
pub fn lcd(ui: &mut Ui, main: &str, caption: &str, w: f32) {
    let pl = pal();
    let h = if caption.is_empty() { 56.0 } else { 70.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect(rect, 8.0, pl.lcd_bg, Stroke::new(2.0, pl.accent));
    let cy = if caption.is_empty() { rect.center().y } else { rect.top() + 26.0 };
    p.text(pos2(rect.center().x, cy), Align2::CENTER_CENTER, main, FontId::monospace(26.0), pl.lcd_text);
    if !caption.is_empty() {
        p.text(pos2(rect.center().x, rect.bottom() - 12.0), Align2::CENTER_CENTER, caption, FontId::monospace(13.0), pl.lcd_text.linear_multiply(0.8));
    }
}

pub fn wattmeter_panel(ui: &mut Ui, power: f64, pf: f64, vrms: f64, irms: f64) {
    lcd(ui, &fmt_reading(power, "W"), &format!("Power factor {:.3}", pf), 280.0);
    ui.add_space(4.0);
    egui::Grid::new("wm_grid").num_columns(2).show(ui, |ui| {
        ui.label("Voltage (RMS)");
        ui.monospace(fmt_reading(vrms, "V"));
        ui.end_row();
        ui.label("Current (RMS)");
        ui.monospace(fmt_reading(irms, "A"));
        ui.end_row();
        ui.label("Apparent power");
        ui.monospace(fmt_reading(vrms * irms, "VA"));
        ui.end_row();
    });
    ui.label(RichText::new("Wire V+ / V− across the load and I+ / I− in series with it.").small().weak());
}

#[derive(Clone, Copy, Default)]
pub struct FreqStats {
    pub freq: f64,
    pub period: f64,
    pub vpp: f64,
    pub duty: f64,
}

/// Measure frequency from rising crossings of `level` in the meter's sample window.
pub fn freq_stats(m: &Meter, level: f64) -> Option<FreqStats> {
    let v: Vec<(f64, f64)> = m.samples.iter().copied().collect();
    if v.len() < 4 {
        return None;
    }
    let mut rises = Vec::new();
    let mut above_time = 0.0;
    for w in v.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a.1 >= level {
            above_time += b.0 - a.0;
        }
        if a.1 < level && b.1 >= level {
            let f = (level - a.1) / (b.1 - a.1).max(1e-30);
            rises.push(a.0 + f * (b.0 - a.0));
        }
    }
    let vpp = m.peak_to_peak();
    if rises.len() < 2 {
        return Some(FreqStats { vpp, ..Default::default() });
    }
    let period = (rises[rises.len() - 1] - rises[0]) / (rises.len() - 1) as f64;
    let span = v[v.len() - 1].0 - v[0].0;
    Some(FreqStats { freq: 1.0 / period, period, vpp, duty: if span > 0.0 { 100.0 * above_time / span } else { 0.0 } })
}

pub fn freq_counter_panel(ui: &mut Ui, id: egui::Id, params: &mut Params, stats: Option<FreqStats>) {
    let st = stats.unwrap_or_default();
    let main = if st.freq > 0.0 { fmt_reading(st.freq, "Hz") } else { "---- Hz".into() };
    lcd(ui, &main, if st.freq > 0.0 { "signal locked" } else { "waiting for signal" }, 280.0);
    ui.add_space(4.0);
    egui::Grid::new(id.with("fc")).num_columns(2).show(ui, |ui| {
        ui.label("Period");
        ui.monospace(if st.period > 0.0 { fmt_reading(st.period, "s") } else { "—".into() });
        ui.end_row();
        ui.label("Peak to peak");
        ui.monospace(fmt_reading(st.vpp, "V"));
        ui.end_row();
        ui.label("Time above level");
        ui.monospace(format!("{:.1} %", st.duty));
        ui.end_row();
        ui.label("Trigger level");
        si_field(ui, id.with("lvl"), &mut params.value, "V", 100.0);
        ui.end_row();
    });
}
