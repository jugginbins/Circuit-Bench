//! The 2D breadboard view: a top-down board with rails and strips, realistic parts and jumper wires.

use super::util::*;
use super::*;
use crate::board::{self, COLS};

const PITCH: f32 = 16.0;
const LEAD: Color32 = Color32::from_rgb(178, 182, 190);
const JUMPER_COLORS: [WireColor; 6] = [WireColor::Red, WireColor::Black, WireColor::Blue, WireColor::Green, WireColor::Orange, WireColor::Purple];

fn bez(a: Pos2, c: Pos2, b: Pos2, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            pos2(u * u * a.x + 2.0 * u * t * c.x + t * t * b.x, u * u * a.y + 2.0 * u * t * c.y + t * t * b.y)
        })
        .collect()
}

fn capsule(p: &egui::Painter, a: Pos2, b: Pos2, w: f32, fill: Color32) {
    let outline = theme::blend(fill, Color32::BLACK, 0.45);
    p.line_segment([a, b], Stroke::new(w + 2.0, outline));
    p.circle_filled(a, w / 2.0 + 1.0, outline);
    p.circle_filled(b, w / 2.0 + 1.0, outline);
    p.line_segment([a, b], Stroke::new(w, fill));
    p.circle_filled(a, w / 2.0, fill);
    p.circle_filled(b, w / 2.0, fill);
    // soft highlight along the top
    let n = (b - a).normalized().rot90();
    p.line_segment([a - n * w * 0.22, b - n * w * 0.22], Stroke::new(w * 0.18, Color32::from_white_alpha(70)));
}

fn band(p: &egui::Painter, a: Pos2, b: Pos2, t: f32, w: f32, col: Color32) {
    let c = a + (b - a) * t;
    let n = (b - a).normalized().rot90();
    p.line_segment([c - n * w * 0.5, c + n * w * 0.5], Stroke::new(w * 0.24, col));
}

fn digit_color(d: i32) -> Color32 {
    match d {
        0 => Color32::from_rgb(20, 20, 20),
        1 => Color32::from_rgb(120, 70, 30),
        2 => Color32::from_rgb(220, 40, 30),
        3 => Color32::from_rgb(245, 130, 20),
        4 => Color32::from_rgb(250, 220, 20),
        5 => Color32::from_rgb(40, 160, 60),
        6 => Color32::from_rgb(40, 90, 220),
        7 => Color32::from_rgb(140, 60, 200),
        8 => Color32::from_rgb(130, 130, 130),
        9 => Color32::from_rgb(245, 245, 245),
        -1 => Color32::from_rgb(212, 175, 55),
        _ => Color32::from_rgb(192, 192, 192),
    }
}

/// Colour bands for a resistor value (two digits, multiplier).
fn resistor_bands(v: f64) -> [Color32; 3] {
    if v <= 0.0 || !v.is_finite() {
        return [digit_color(0); 3];
    }
    let mut e = v.log10().floor() as i32 - 1;
    let mut d = (v / 10f64.powi(e)).round() as i32;
    if d >= 100 {
        d /= 10;
        e += 1;
    }
    let d = d.clamp(10, 99);
    [digit_color(d / 10), digit_color(d % 10), digit_color(e.clamp(-2, 9))]
}

impl App {
    fn bsize(&self) -> f32 {
        PITCH * self.bzoom
    }

    fn fit_board(&mut self, rect: Rect) {
        let (mut x0, mut y0, mut x1, mut y1) = (-2.5f32, -2.0f32, COLS as f32 + 1.5, 18.0f32);
        for p in &self.circuit.board.parts {
            if let Some((bx, by)) = p.body {
                x0 = x0.min(bx - 3.0);
                x1 = x1.max(bx + 3.0);
                y0 = y0.min(by - 3.5);
                y1 = y1.max(by + 3.5);
            }
        }
        let z = ((rect.width() - 40.0) / ((x1 - x0) * PITCH)).min((rect.height() - 40.0) / ((y1 - y0) * PITCH)).clamp(0.4, 3.0);
        self.bzoom = z;
        let u = PITCH * z;
        self.bpan = rect.center() - rect.min - vec2((x0 + x1) / 2.0 * u, (y0 + y1) / 2.0 * u);
    }

    pub(super) fn board_view(&mut self, ui: &mut Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        if self.frame < 6 {
            self.bfit = true;
        }
        if self.bfit {
            self.fit_board(rect);
            self.bfit = false;
        }
        if resp.hovered() {
            let (scroll, zd, hover) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
            if zd != 1.0 {
                if let Some(m) = hover {
                    let world = (m - rect.min - self.bpan) / self.bsize();
                    self.bzoom = (self.bzoom * zd).clamp(0.3, 5.0);
                    self.bpan = m - rect.min - world * self.bsize();
                }
            } else {
                self.bpan += scroll;
            }
        }
        if resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Secondary) {
            self.bpan += resp.drag_delta();
        }
        let u = self.bsize();
        let origin = rect.min + self.bpan;
        let hs = |h: BH| origin + vec2(h.x as f32 * u, h.y as f32 * u);
        let ws = |x: f32, y: f32| origin + vec2(x * u, y * u);
        let pl = pal();
        let dt = ui.input(|i| i.stable_dt).clamp(0.001, 0.05);

        // bench background with a soft vignette grid
        painter.rect_filled(rect, 0.0, theme::blend(pl.sheet, pl.panel, 0.35));
        let step = 4.0 * u;
        if step > 12.0 {
            let mut x = origin.x.rem_euclid(step) + rect.left();
            while x < rect.right() {
                painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(0.5, pl.grid.linear_multiply(0.35)));
                x += step;
            }
            let mut y = origin.y.rem_euclid(step) + rect.top();
            while y < rect.bottom() {
                painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(0.5, pl.grid.linear_multiply(0.35)));
                y += step;
            }
        }

        // ---- the board
        let br = Rect::from_min_max(ws(-1.7, -1.4), ws(COLS as f32 + 0.7, 17.4));
        painter.rect_filled(br.translate(vec2(0.0, 0.35 * u)).expand(2.0), 0.6 * u, Color32::from_black_alpha(if pl.dark { 120 } else { 45 }));
        painter.rect_filled(br, 0.6 * u, Color32::from_rgb(246, 243, 235));
        painter.rect_stroke(br, 0.6 * u, Stroke::new(1.0, Color32::from_rgb(214, 208, 196)));
        // rail stripes and labels
        let red = Color32::from_rgb(220, 50, 45);
        let blue = Color32::from_rgb(40, 90, 210);
        for (y, col) in [(-0.7, blue), (1.7, red), (14.3, red), (16.7, blue)] {
            painter.line_segment([ws(-0.6, y), ws(COLS as f32 - 0.4, y)], Stroke::new((0.1 * u).max(1.0), col));
        }
        let fs = (0.55 * u).clamp(6.0, 22.0);
        for (y, plus) in [(0.0, false), (1.0, true), (15.0, true), (16.0, false)] {
            let (t, col) = if plus { ("+", red) } else { ("−", blue) };
            painter.text(ws(-1.15, y), Align2::CENTER_CENTER, t, FontId::proportional(fs * 1.2), col);
            painter.text(ws(COLS as f32 + 0.15, y), Align2::CENTER_CENTER, t, FontId::proportional(fs * 1.2), col);
        }
        // centre channel
        painter.rect_filled(Rect::from_min_max(ws(-1.7, 7.55), ws(COLS as f32 + 0.7, 8.45)), 0.0, Color32::from_rgb(226, 221, 210));
        // row letters and column numbers
        let lab = Color32::from_rgb(120, 116, 108);
        for y in (3..=7).chain(9..=13) {
            painter.text(ws(-1.0, y as f32), Align2::CENTER_CENTER, board::row_label(y), FontId::proportional(fs * 0.85), lab);
            painter.text(ws(COLS as f32 - 0.05, y as f32), Align2::CENTER_CENTER, board::row_label(y), FontId::proportional(fs * 0.85), lab);
        }
        for x in 0..COLS {
            if x == 0 || (x + 1) % 5 == 0 {
                painter.text(ws(x as f32, 2.15), Align2::CENTER_CENTER, (x + 1).to_string(), FontId::proportional(fs * 0.75), lab);
                painter.text(ws(x as f32, 13.85), Align2::CENTER_CENTER, (x + 1).to_string(), FontId::proportional(fs * 0.75), lab);
            }
        }

        // ---- hover
        let hover = resp.hover_pos();
        let hw = hover.map(|q| ((q.x - origin.x) / u, (q.y - origin.y) / u));
        let hover_hole = hw.map(|(x, y)| BH::new(x.round() as i32, y.round() as i32)).filter(|h| board::is_hole(*h)).filter(|h| {
            let (x, y) = hw.unwrap();
            ((h.x as f32 - x).powi(2) + (h.y as f32 - y).powi(2)).sqrt() < 0.55
        });
        // highlight the strip under the pointer so you can see what is joined
        if let Some(h) = hover_hole {
            let holes = board::strip_holes(h);
            let a = hs(holes[0]);
            let b = hs(*holes.last().unwrap());
            painter.rect_filled(Rect::from_two_pos(a, b).expand(0.42 * u), 0.4 * u, pl.accent.linear_multiply(0.18));
        }
        // holes
        let occupied = board::occupied(&self.circuit.board, None, None);
        let hole_r = (0.19 * u).max(1.5);
        for x in 0..COLS {
            for y in [0, 1, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 15, 16] {
                let c = hs(BH::new(x, y));
                painter.rect_filled(Rect::from_center_size(c, vec2(hole_r * 2.0, hole_r * 2.0)), hole_r * 0.4, Color32::from_rgb(70, 68, 64));
                if occupied.contains(&BH::new(x, y)) {
                    painter.circle_filled(c, hole_r * 0.7, Color32::from_rgb(150, 150, 155));
                }
            }
        }
        if let Some(h) = hover_hole {
            let col = if occupied.contains(&h) && matches!(self.btool, BTool::Jumper | BTool::Place(_)) { Color32::from_rgb(230, 60, 50) } else { pl.accent };
            painter.circle_stroke(hs(h), hole_r * 2.2, Stroke::new(2.0, col));
            self.hover_info = format!("Hole {}{}", board::row_label(h.y).to_uppercase(), if board::is_rail(h.y) { format!(" rail {}", if board::rail_is_plus(h.y) { "+" } else { "−" }) } else { (h.x + 1).to_string() });
            if self.sim_board {
                if let Some(sim) = &self.sim {
                    // find a part pin on this conductor to read its voltage
                    let n = board::node(h);
                    for (pi, p) in self.circuit.board.parts.iter().enumerate() {
                        if let Some(k) = p.pins.iter().position(|q| board::node(*q) == n) {
                            if let Some(net) = sim.nl.comp_nets.get(pi).map(|v| v[k]) {
                                self.hover_info = format!("{}  ·  {}", self.hover_info, fmt_reading(sim.v(net), "V"));
                            }
                            break;
                        }
                    }
                }
            }
        }

        // ---- parts and wires
        let sel = self.bsel;
        for (pi, part) in self.circuit.board.parts.iter().enumerate() {
            let live = self.live.get(&part.comp.id).cloned().unwrap_or_default();
            let selected = sel == Some(BSel::Part(part.comp.id));
            draw_part(&painter, part, &hs, &ws, u, &live, selected);
            let _ = pi;
        }
        for j in &self.circuit.board.jumpers {
            draw_jumper(&painter, hs(j.a), hs(j.b), j.color, u, sel == Some(BSel::Jumper(j.id)));
        }
        // flying-lead bodies are drawn after jumpers so their leads stay on top
        for part in self.circuit.board.parts.iter().filter(|p| board::is_flying(p.comp.kind)) {
            let live = self.live.get(&part.comp.id).cloned().unwrap_or_default();
            draw_flying_body(&painter, part, &hs, &ws, u, &live, sel == Some(BSel::Part(part.comp.id)));
        }

        // ---- previews
        match self.btool {
            BTool::Jumper => {
                if let (Some(a), Some(m)) = (self.jumper_start, hover) {
                    let col = JUMPER_COLORS[self.jcolor % JUMPER_COLORS.len()];
                    draw_jumper(&painter, hs(a), hover_hole.map(hs).unwrap_or(m), col, u, false);
                }
            }
            BTool::Place(k) => {
                let ghost = pl.accent;
                if board::is_flying(k) {
                    if let Some((bx, by)) = self.bbody {
                        painter.rect_stroke(Rect::from_center_size(ws(bx, by), vec2(3.0 * u, 2.4 * u)), 6.0, Stroke::new(2.0, ghost));
                    } else if let Some((x, y)) = hw {
                        painter.rect_stroke(Rect::from_center_size(ws(x, y), vec2(3.0 * u, 2.4 * u)), 6.0, Stroke::new(1.5, ghost.linear_multiply(0.6)));
                    }
                }
                for h in &self.bplace {
                    painter.circle_filled(hs(*h), hole_r * 1.6, ghost);
                }
                if let (Some(a), Some(m)) = (self.bplace.last(), hover) {
                    painter.line_segment([hs(*a), hover_hole.map(hs).unwrap_or(m)], Stroke::new(0.25 * u, ghost.linear_multiply(0.6)));
                } else if let (Some((bx, by)), Some(m)) = (self.bbody, hover) {
                    painter.line_segment([ws(bx, by), hover_hole.map(hs).unwrap_or(m)], Stroke::new(0.2 * u, ghost.linear_multiply(0.6)));
                }
                if let (Some(h), Some(pins)) = (hover_hole, hover_hole.and_then(|h| board::auto_pins(k, h))) {
                    let _ = h;
                    for q in pins {
                        painter.circle_stroke(hs(q), hole_r * 1.8, Stroke::new(2.0, ghost));
                    }
                }
            }
            BTool::Select => {}
        }

        // ---- input
        let (pressed, released, down, ptr) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_released(), i.pointer.primary_down(), i.pointer.interact_pos()));
        let hit = hw.and_then(|(x, y)| self.board_hit(x, y));
        if pressed && resp.hovered() {
            if let Some((x, y)) = hw {
                self.board_press(x, y, hover_hole, hit, ptr);
            }
        }
        if down {
            if let (Some((x, y)), Some(d)) = (hw, self.bdrag.as_mut()) {
                d.cur = (x, y);
            }
            self.board_drag_update();
        }
        if released {
            if let Some(d) = self.bdrag.take() {
                if d.moved {
                    self.push_undo(d.orig);
                    self.edit_stop();
                }
            }
        }
        if resp.double_clicked() {
            if let Some(BSel::Part(id)) = hit {
                self.props = Some(id);
                self.props_orig = Some(self.circuit.clone());
            }
        }
        if resp.secondary_clicked() {
            self.bplace.clear();
            self.bbody = None;
            self.jumper_start = None;
        }

        // empty-board helper
        if self.circuit.board.parts.is_empty() && self.circuit.board.jumpers.is_empty() {
            let r = Rect::from_center_size(rect.center_bottom() - vec2(0.0, 70.0), vec2(420.0, 70.0));
            painter.rect(r, 10.0, pl.window, Stroke::new(1.0, pl.accent));
            painter.text(r.center() - vec2(0.0, 12.0), Align2::CENTER_CENTER, "The breadboard is empty", FontId::proportional(16.0), pl.text);
            painter.text(r.center() + vec2(0.0, 12.0), Align2::CENTER_CENTER, "Pick a part on the left, or press “Starter board” above", FontId::proportional(13.0), pl.weak_text);
        }
        if self.running && self.sim_board {
            let badge = Rect::from_min_size(rect.left_top() + vec2(8.0, 8.0), vec2(170.0, 22.0));
            let pulse = 0.5 + 0.5 * (self.anim_clock * 3.0).sin();
            let col = if self.paused { Color32::from_rgb(220, 160, 0) } else { Color32::from_rgb(30, 160, 60) };
            painter.rect_filled(badge.expand(2.0 + 2.0 * pulse), 8.0, Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), (40.0 * pulse) as u8));
            painter.rect_filled(badge, 6.0, col);
            painter.text(badge.center(), Align2::CENTER_CENTER, if self.paused { "BOARD PAUSED" } else { "BOARD POWERED" }, FontId::proportional(12.0), Color32::WHITE);
        }
        let _ = dt;
    }

    /// Which part or jumper is under a board point.
    fn board_hit(&self, x: f32, y: f32) -> Option<BSel> {
        let d_seg = |p: (f32, f32), a: (f32, f32), b: (f32, f32)| {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l2 = dx * dx + dy * dy;
            let t = if l2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
            ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
        };
        let f = |h: BH| (h.x as f32, h.y as f32);
        for p in self.circuit.board.parts.iter().rev() {
            if let Some((bx, by)) = p.body {
                let (w, h) = flying_size(p.comp.kind);
                if (x - bx).abs() <= w / 2.0 && (y - by).abs() <= h / 2.0 {
                    return Some(BSel::Part(p.comp.id));
                }
                continue;
            }
            let holes = board::chip_holes(p);
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for h in &holes {
                x0 = x0.min(h.x as f32);
                y0 = y0.min(h.y as f32);
                x1 = x1.max(h.x as f32);
                y1 = y1.max(h.y as f32);
            }
            let hit = if p.pins.len() == 2 {
                d_seg((x, y), f(p.pins[0]), f(p.pins[1])) < 0.55
            } else {
                x >= x0 - 0.6 && x <= x1 + 0.6 && y >= y0 - 0.9 && y <= y1 + 0.6
            };
            if hit {
                return Some(BSel::Part(p.comp.id));
            }
        }
        for j in self.circuit.board.jumpers.iter().rev() {
            if d_seg((x, y), f(j.a), f(j.b)) < 0.5 {
                return Some(BSel::Jumper(j.id));
            }
        }
        None
    }

    fn board_press(&mut self, x: f32, y: f32, hole: Option<BH>, hit: Option<BSel>, _ptr: Option<Pos2>) {
        match self.btool {
            BTool::Jumper => {
                let Some(h) = hole else {
                    return;
                };
                let occ = board::occupied(&self.circuit.board, None, None);
                if occ.contains(&h) {
                    self.log("That hole is already in use — pick an empty one.");
                    return;
                }
                match self.jumper_start {
                    None => self.jumper_start = Some(h),
                    Some(a) if a != h => {
                        self.edit_stop();
                        self.checkpoint();
                        let id = self.circuit.new_id();
                        let color = JUMPER_COLORS[self.jcolor % JUMPER_COLORS.len()];
                        self.circuit.board.jumpers.push(Jumper { id, a, b: h, color });
                        self.jumper_start = None;
                    }
                    _ => {}
                }
            }
            BTool::Place(kind) => {
                let occ = board::occupied(&self.circuit.board, None, None);
                if board::is_flying(kind) && self.bbody.is_none() {
                    self.bbody = Some(((x * 2.0).round() / 2.0, (y * 2.0).round() / 2.0));
                    return;
                }
                let Some(h) = hole else {
                    return;
                };
                if let Some(pins) = board::auto_pins(kind, h) {
                    let mut all = pins.clone();
                    if kind == Kind::OpAmp {
                        let probe = BoardPart { comp: Component { id: 0, kind, refdes: String::new(), pos: GP::new(0, 0), rot: 0, params: Params::default() }, pins: pins.clone(), body: None };
                        all = board::chip_holes(&probe);
                    }
                    if all.iter().any(|q| !board::is_hole(*q) || occ.contains(q)) {
                        self.log("The part does not fit there — it needs free holes in a row (op-amps straddle the centre channel).");
                        return;
                    }
                    self.add_board_part(kind, pins, None);
                    return;
                }
                if occ.contains(&h) || self.bplace.contains(&h) {
                    self.log("That hole is already in use — pick an empty one.");
                    return;
                }
                self.bplace.push(h);
                if self.bplace.len() >= 2 {
                    let pins = std::mem::take(&mut self.bplace);
                    let body = self.bbody.take();
                    self.add_board_part(kind, pins, body);
                }
            }
            BTool::Select => {
                self.bsel = hit;
                if let Some(BSel::Part(id)) = hit {
                    if self.running {
                        if let Some(c) = self.circuit.comp_any_mut(id) {
                            match c.kind {
                                Kind::Switch => c.params.closed = !c.params.closed,
                                Kind::PushButton => self.held_button = Some(id),
                                _ => {}
                            }
                        }
                    }
                }
                if hit.is_some() {
                    self.bdrag = Some(BDrag { start: (x, y), cur: (x, y), orig: self.circuit.clone(), moved: false });
                }
            }
        }
    }

    fn add_board_part(&mut self, kind: Kind, pins: Vec<BH>, body: Option<(f32, f32)>) {
        self.edit_stop();
        self.checkpoint();
        let id = self.circuit.new_id();
        let refdes = board::next_refdes(&self.circuit.board, kind);
        let mut params = kind.default_params();
        if kind == Kind::Potentiometer {
            params.value = 1000.0;
        }
        if kind == Kind::Resistor {
            params.value = 330.0;
        }
        if kind == Kind::DcVoltage {
            params.value = 9.0;
        }
        self.circuit.board.parts.push(BoardPart { comp: Component { id, kind, refdes, pos: GP::new(0, 0), rot: 0, params }, pins, body });
        self.bsel = Some(BSel::Part(id));
        if !self.ctx_shift {
            self.btool = BTool::Select;
        }
    }

    fn board_drag_update(&mut self) {
        let Some(d) = &mut self.bdrag else {
            return;
        };
        let Some(BSel::Part(id)) = self.bsel else {
            return;
        };
        let dx = d.cur.0 - d.start.0;
        let dy = d.cur.1 - d.start.1;
        let mut c = d.orig.clone();
        let Some(pi) = c.board.parts.iter().position(|p| p.comp.id == id) else {
            return;
        };
        if let Some((bx, by)) = c.board.parts[pi].body {
            // flying parts: move the body freely, the leads stay plugged in
            if dx.abs() + dy.abs() < 0.2 {
                return;
            }
            c.board.parts[pi].body = Some((((bx + dx) * 2.0).round() / 2.0, ((by + dy) * 2.0).round() / 2.0));
        } else {
            let (sx, sy) = (dx.round() as i32, dy.round() as i32);
            if sx == 0 && sy == 0 {
                self.circuit = d.orig.clone();
                return;
            }
            let occ = board::occupied(&c.board, Some(id), None);
            let moved: Vec<BH> = c.board.parts[pi].pins.iter().map(|h| BH::new(h.x + sx, h.y + sy)).collect();
            let mut probe = c.board.parts[pi].clone();
            probe.pins = moved.clone();
            if board::chip_holes(&probe).iter().any(|h| !board::is_hole(*h) || occ.contains(h)) {
                return;
            }
            c.board.parts[pi].pins = moved;
        }
        d.moved = true;
        self.circuit = c;
    }

    pub(super) fn board_delete(&mut self) {
        let Some(s) = self.bsel.take() else {
            return;
        };
        self.edit_stop();
        self.checkpoint();
        match s {
            BSel::Part(id) => self.circuit.board.parts.retain(|p| p.comp.id != id),
            BSel::Jumper(id) => self.circuit.board.jumpers.retain(|j| j.id != id),
        }
    }

    /// Parts bin for the breadboard view.
    pub(super) fn board_parts_panel(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.heading("Breadboard parts");
        ui.label(RichText::new("Click a part, then click holes on the board. Batteries, meters and motors: click a spot for the body, then two holes for the leads.").small().weak());
        ui.add_space(2.0);
        egui::ScrollArea::vertical().id_source("bparts_scroll").show(ui, |ui| {
            for (group, kinds) in board::BOARD_PARTS {
                egui::CollapsingHeader::new(RichText::new(*group).strong()).id_source(("bg", *group)).default_open(true).show(ui, |ui| {
                    for k in kinds.iter() {
                        if part_row(ui, *k, self.btool == BTool::Place(*k)).clicked() {
                            self.btool = BTool::Place(*k);
                            self.bplace.clear();
                            self.bbody = None;
                        }
                    }
                });
            }
            ui.separator();
            ui.label(RichText::new("On this board").strong());
            let items: Vec<(String, u64, Kind)> = self.circuit.board.parts.iter().map(|p| (p.comp.refdes.clone(), p.comp.id, p.comp.kind)).collect();
            for (name, id, kind) in items {
                let r = ui.selectable_label(self.bsel == Some(BSel::Part(id)), format!("    {}  —  {}", name, kind.name()));
                if r.clicked() {
                    self.bsel = Some(BSel::Part(id));
                }
                if r.double_clicked() {
                    self.props = Some(id);
                    self.props_orig = Some(self.circuit.clone());
                }
            }
        });
    }

    /// Tool strip shown above the breadboard.
    pub(super) fn board_tools(&mut self, ui: &mut Ui) {
        let pl = pal();
        if ui.add(egui::Button::new("Select").selected(self.btool == BTool::Select)).clicked() {
            self.btool = BTool::Select;
            self.bplace.clear();
            self.bbody = None;
            self.jumper_start = None;
        }
        if ui.add(egui::Button::new("Jumper wire").selected(self.btool == BTool::Jumper)).on_hover_text("Click two holes to join them").clicked() {
            self.btool = BTool::Jumper;
            self.jumper_start = None;
        }
        for (k, c) in JUMPER_COLORS.iter().enumerate() {
            let [r, g, b] = c.rgb();
            let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            ui.painter().circle_filled(rect.center(), 7.0, theme::wire_color([r, g, b]));
            if self.jcolor == k {
                ui.painter().circle_stroke(rect.center(), 8.5, Stroke::new(2.0, pl.text));
            }
            if resp.on_hover_text(format!("{} jumper", c.name())).clicked() {
                self.jcolor = k;
                self.btool = BTool::Jumper;
            }
        }
        ui.separator();
        if ui.button("Starter board").on_hover_text("Fill the board with a ready-made set of small circuits").clicked() {
            self.edit_stop();
            self.checkpoint();
            board::starter_board(&mut self.circuit);
            self.bfit = true;
        }
        if ui.button("Clear board").clicked() && !(self.circuit.board.parts.is_empty() && self.circuit.board.jumpers.is_empty()) {
            self.edit_stop();
            self.checkpoint();
            self.circuit.board = Board::default();
        }
        if ui.button("Fit").clicked() {
            self.bfit = true;
        }
    }
}

fn flying_size(k: Kind) -> (f32, f32) {
    match k {
        Kind::DcVoltage => (3.2, 2.4),
        Kind::ClockSource => (4.2, 2.8),
        Kind::Motor => (3.0, 3.0),
        Kind::Multimeter => (3.8, 5.2),
        _ => (3.0, 2.4),
    }
}

fn draw_jumper(p: &egui::Painter, a: Pos2, b: Pos2, color: WireColor, u: f32, selected: bool) {
    let d = b - a;
    let len = d.length().max(1.0);
    let n = if d.x.abs() >= d.y.abs() { vec2(0.0, -1.0) } else { vec2(-1.0, 0.0) };
    let c = a + d * 0.5 + n * (len * 0.18).min(1.4 * u);
    let pts = bez(a, c, b, 24);
    let [r, g, bl] = color.rgb();
    let col = if color == WireColor::Black { Color32::from_rgb(45, 45, 50) } else { Color32::from_rgb(r, g, bl) };
    if selected {
        p.add(Shape::line(pts.clone(), Stroke::new(0.55 * u + 4.0, pal().select.linear_multiply(0.5))));
    }
    p.add(Shape::line(pts.iter().map(|q| *q + vec2(0.0, 0.12 * u)).collect(), Stroke::new(0.3 * u, Color32::from_black_alpha(50))));
    p.add(Shape::line(pts.clone(), Stroke::new(0.3 * u, theme::blend(col, Color32::BLACK, 0.4))));
    p.add(Shape::line(pts.clone(), Stroke::new(0.22 * u, col)));
    p.add(Shape::line(pts.iter().map(|q| *q - vec2(0.0, 0.05 * u)).collect(), Stroke::new(0.05 * u, Color32::from_white_alpha(90))));
    for e in [a, b] {
        p.circle_filled(e, 0.16 * u, LEAD);
    }
}

fn lead(p: &egui::Painter, a: Pos2, b: Pos2, u: f32) {
    p.line_segment([a, b], Stroke::new((0.12 * u).max(1.2), LEAD));
    p.circle_filled(a, 0.13 * u, Color32::from_rgb(140, 140, 148));
}

fn draw_part(p: &egui::Painter, part: &BoardPart, hs: &dyn Fn(BH) -> Pos2, ws: &dyn Fn(f32, f32) -> Pos2, u: f32, live: &Live, selected: bool) {
    let k = part.comp.kind;
    let prm = &part.comp.params;
    let glow = live.glow.clamp(0.0, 1.0);
    let sel_col = pal().select;
    if board::is_flying(k) {
        // only the leads here; bodies are drawn later
        if let Some((bx, by)) = part.body {
            let (w, h) = flying_size(k);
            for (i, hole) in part.pins.iter().enumerate().take(2) {
                let target = hs(*hole);
                let sign = if target.y >= ws(bx, by).y { 1.0 } else { -1.0 };
                let anchor = ws(bx + (i as f32 - 0.5) * w * 0.45, by + sign * h / 2.0);
                let ctrl = pos2((anchor.x + target.x) / 2.0, anchor.y.max(target.y) + 1.5 * u);
                let col = if i == 0 { Color32::from_rgb(210, 40, 35) } else { Color32::from_rgb(40, 40, 45) };
                let pts = bez(anchor, ctrl, target, 24);
                p.add(Shape::line(pts.clone(), Stroke::new(0.3 * u, theme::blend(col, Color32::BLACK, 0.35))));
                p.add(Shape::line(pts, Stroke::new(0.22 * u, col)));
                p.circle_filled(target, 0.2 * u, if k == Kind::Multimeter { LEAD } else { theme::blend(col, Color32::BLACK, 0.2) });
            }
        }
        return;
    }
    let ptxt = |pos: Pos2, t: &str, size: f32, col: Color32| {
        p.text(pos, Align2::CENTER_CENTER, t, FontId::proportional((size * u).clamp(6.0, 26.0)), col);
    };
    match k {
        Kind::OpAmp => {
            let holes = board::chip_holes(part);
            let x0 = holes[0].x as f32;
            let r = Rect::from_min_max(ws(x0 - 0.45, 7.25), ws(x0 + 3.45, 8.75));
            for h in &holes {
                let c = hs(*h);
                let toward = if h.y == 7 { vec2(0.0, 0.25 * u) } else { vec2(0.0, -0.25 * u) };
                p.line_segment([c, c + toward], Stroke::new(0.22 * u, LEAD));
            }
            p.rect_filled(r.translate(vec2(0.0, 0.12 * u)), 0.15 * u, Color32::from_black_alpha(60));
            p.rect_filled(r, 0.15 * u, Color32::from_rgb(38, 38, 42));
            p.circle_filled(r.left_center(), 0.28 * u, Color32::from_rgb(70, 70, 76));
            p.circle_filled(r.left_center() + vec2(0.55 * u, 0.4 * u), 0.08 * u, Color32::from_rgb(90, 90, 96));
            ptxt(r.center(), &part.comp.refdes, 0.5, Color32::from_rgb(200, 200, 205));
            if selected {
                p.rect_stroke(r.expand(3.0), 4.0, Stroke::new(2.0, sel_col));
            }
        }
        Kind::Potentiometer => {
            for h in &part.pins {
                lead(p, hs(*h), hs(*h) - vec2(0.0, 0.6 * u), u);
            }
            let mid = hs(part.pins[2]) - vec2(0.0, 1.2 * u);
            let r = Rect::from_center_size(mid, vec2(2.6 * u, 1.8 * u));
            p.rect_filled(r.translate(vec2(0.0, 0.12 * u)), 0.25 * u, Color32::from_black_alpha(60));
            p.rect_filled(r, 0.25 * u, Color32::from_rgb(40, 90, 200));
            p.circle_filled(mid, 0.7 * u, Color32::from_rgb(235, 235, 240));
            let a = (-135.0 + 270.0 * (prm.setting as f32 / 100.0)).to_radians() - std::f32::consts::FRAC_PI_2;
            p.line_segment([mid, mid + vec2(a.cos(), a.sin()) * 0.6 * u], Stroke::new(0.18 * u, Color32::from_rgb(60, 60, 70)));
            if selected {
                p.rect_stroke(r.expand(3.0), 4.0, Stroke::new(2.0, sel_col));
            }
        }
        _ => {
            let a = hs(part.pins[0]);
            let b = hs(part.pins[1]);
            let m = a + (b - a) * 0.5;
            let len = (b - a).length().max(1.0);
            let dir = (b - a) / len;
            let bl = (len - 0.6 * u).clamp(0.4 * u, 2.4 * u);
            let s0 = m - dir * bl / 2.0;
            let s1 = m + dir * bl / 2.0;
            if selected {
                p.line_segment([a, b], Stroke::new(1.2 * u, sel_col.linear_multiply(0.35)));
            }
            match k {
                Kind::Resistor | Kind::Inductor => {
                    lead(p, a, s0, u);
                    lead(p, b, s1, u);
                    let body = if k == Kind::Resistor { Color32::from_rgb(222, 196, 150) } else { Color32::from_rgb(70, 140, 90) };
                    capsule(p, s0 + dir * 0.15 * u, s1 - dir * 0.15 * u, 0.55 * u, body);
                    if k == Kind::Resistor {
                        let bands = resistor_bands(prm.value);
                        for (i, t) in [0.22f32, 0.37, 0.52].iter().enumerate() {
                            band(p, s0, s1, *t, 0.62 * u, bands[i]);
                        }
                        band(p, s0, s1, 0.8, 0.62 * u, digit_color(-1));
                    } else {
                        for t in [0.3f32, 0.5, 0.7] {
                            band(p, s0, s1, t, 0.6 * u, Color32::from_rgb(200, 170, 60));
                        }
                    }
                }
                Kind::Capacitor => {
                    lead(p, a, m, u);
                    lead(p, b, m, u);
                    if prm.value >= 1e-6 {
                        // electrolytic can seen from above
                        p.circle_filled(m + vec2(0.0, 0.1 * u), 0.72 * u, Color32::from_black_alpha(60));
                        p.circle_filled(m, 0.7 * u, Color32::from_rgb(40, 60, 150));
                        p.circle_stroke(m, 0.7 * u, Stroke::new(1.0, Color32::from_rgb(20, 30, 80)));
                        let n = dir * 0.45 * u;
                        p.line_segment([m + n + dir.rot90() * 0.45 * u, m + n - dir.rot90() * 0.45 * u], Stroke::new(0.18 * u, Color32::from_rgb(200, 210, 235)));
                        p.circle_stroke(m, 0.3 * u, Stroke::new(0.08 * u, Color32::from_rgb(160, 170, 200)));
                    } else {
                        p.circle_filled(m + vec2(0.0, 0.1 * u), 0.5 * u, Color32::from_black_alpha(60));
                        p.circle_filled(m, 0.48 * u, Color32::from_rgb(232, 150, 50));
                        p.circle_filled(m - vec2(0.12, 0.14) * u, 0.15 * u, Color32::from_white_alpha(80));
                    }
                }
                Kind::Led => {
                    lead(p, a, m, u);
                    lead(p, b, m, u);
                    let [r, g, bb] = prm.led.rgb();
                    let col = Color32::from_rgb(r, g, bb);
                    if glow > 0.02 {
                        for i in 0..6 {
                            let f = 1.0 - i as f32 / 6.0;
                            p.circle_filled(m, (0.6 + 2.4 * glow * (i as f32 + 1.0) / 6.0) * u, Color32::from_rgba_unmultiplied(r, g, bb, (glow * 60.0 * f) as u8));
                        }
                    }
                    let body = theme::blend(theme::blend(col, Color32::from_rgb(60, 60, 60), 0.45), col, glow);
                    p.circle_filled(m + vec2(0.0, 0.1 * u), 0.62 * u, Color32::from_black_alpha(60));
                    p.circle_filled(m, 0.6 * u, body);
                    p.circle_stroke(m, 0.6 * u, Stroke::new(1.0, theme::blend(col, Color32::BLACK, 0.4)));
                    p.circle_filled(m - vec2(0.18, 0.2) * u, 0.18 * u, Color32::from_white_alpha((90.0 + 140.0 * glow) as u8));
                    // flat edge marks the cathode
                    let fe = m + dir * 0.55 * u;
                    p.line_segment([fe + dir.rot90() * 0.35 * u, fe - dir.rot90() * 0.35 * u], Stroke::new(0.12 * u, theme::blend(col, Color32::BLACK, 0.5)));
                }
                Kind::Diode | Kind::Zener => {
                    lead(p, a, s0, u);
                    lead(p, b, s1, u);
                    let (body, stripe) = if k == Kind::Diode { (Color32::from_rgb(35, 35, 38), Color32::from_rgb(200, 200, 205)) } else { (Color32::from_rgb(225, 120, 70), Color32::from_rgb(30, 30, 30)) };
                    capsule(p, s0 + dir * 0.2 * u, s1 - dir * 0.2 * u, 0.42 * u, body);
                    band(p, s0, s1, 0.78, 0.5 * u, stripe);
                }
                Kind::Switch => {
                    lead(p, a, m, u);
                    lead(p, b, m, u);
                    let r = Rect::from_center_size(m, vec2(1.8 * u, 0.9 * u));
                    p.rect_filled(r.translate(vec2(0.0, 0.12 * u)), 0.15 * u, Color32::from_black_alpha(60));
                    p.rect_filled(r, 0.15 * u, Color32::from_rgb(60, 70, 90));
                    let slot = r.shrink2(vec2(0.25 * u, 0.25 * u));
                    p.rect_filled(slot, 0.1 * u, Color32::from_rgb(30, 34, 44));
                    let kx = if prm.closed { slot.right() - 0.3 * u } else { slot.left() + 0.3 * u };
                    p.rect_filled(Rect::from_center_size(pos2(kx, slot.center().y), vec2(0.55 * u, 0.55 * u)), 0.1 * u, if prm.closed { pal().accent } else { Color32::from_rgb(220, 220, 225) });
                }
                Kind::PushButton => {
                    lead(p, a, m, u);
                    lead(p, b, m, u);
                    let r = Rect::from_center_size(m, vec2(1.5 * u, 1.5 * u));
                    p.rect_filled(r.translate(vec2(0.0, 0.12 * u)), 0.2 * u, Color32::from_black_alpha(60));
                    p.rect_filled(r, 0.2 * u, Color32::from_rgb(40, 40, 44));
                    let on = prm.closed != prm.normally_closed;
                    let br = if on { 0.42 } else { 0.5 };
                    p.circle_filled(m, br * u, if on { theme::blend(Color32::from_rgb(220, 60, 50), Color32::BLACK, 0.25) } else { Color32::from_rgb(220, 60, 50) });
                    p.circle_filled(m - vec2(0.12, 0.14) * u, 0.12 * u, Color32::from_white_alpha(if on { 40 } else { 110 }));
                }
                Kind::Lamp => {
                    lead(p, a, m, u);
                    lead(p, b, m, u);
                    if glow > 0.02 {
                        for i in 0..6 {
                            let f = 1.0 - i as f32 / 6.0;
                            p.circle_filled(m, (0.8 + 2.6 * glow * (i as f32 + 1.0) / 6.0) * u, Color32::from_rgba_unmultiplied(255, 210, 80, (glow * 70.0 * f) as u8));
                        }
                    }
                    p.circle_filled(m, 0.8 * u, theme::blend(Color32::from_rgb(236, 236, 228), Color32::from_rgb(255, 236, 140), glow));
                    p.circle_stroke(m, 0.8 * u, Stroke::new(1.0, Color32::from_rgb(160, 160, 150)));
                    let fil = theme::blend(Color32::from_rgb(120, 110, 90), Color32::from_rgb(255, 160, 40), glow);
                    p.line_segment([m - dir * 0.4 * u, m + dir * 0.4 * u], Stroke::new(0.08 * u, fil));
                }
                Kind::Buzzer => {
                    lead(p, a, m, u);
                    lead(p, b, m, u);
                    if glow > 0.05 {
                        for i in 0..3 {
                            let ph = (live.time * 2.5 + i as f32 / 3.0).fract();
                            p.circle_stroke(m, (0.9 + ph * 1.6) * u, Stroke::new(0.08 * u, Color32::from_rgba_unmultiplied(pal().accent.r(), pal().accent.g(), pal().accent.b(), ((1.0 - ph) * 200.0 * glow) as u8)));
                        }
                    }
                    let jit = if glow > 0.05 { vec2((live.time * 90.0).sin() * 0.04 * u, 0.0) } else { vec2(0.0, 0.0) };
                    p.circle_filled(m + vec2(0.0, 0.12 * u), 0.92 * u, Color32::from_black_alpha(60));
                    p.circle_filled(m + jit, 0.9 * u, Color32::from_rgb(30, 30, 34));
                    p.circle_stroke(m + jit, 0.6 * u, Stroke::new(0.06 * u, Color32::from_rgb(70, 70, 76)));
                    p.circle_filled(m + jit, 0.18 * u, Color32::from_rgb(10, 10, 12));
                }
                _ => {
                    lead(p, a, b, u);
                }
            }
        }
    }
    // reference label
    if !matches!(k, Kind::OpAmp) && !board::is_flying(k) {
        let (mut x0, mut y0) = (f32::MAX, f32::MAX);
        for h in &part.pins {
            let q = hs(*h);
            x0 = x0.min(q.x);
            y0 = y0.min(q.y);
        }
        let center_x = part.pins.iter().map(|h| hs(*h).x).sum::<f32>() / part.pins.len() as f32;
        let ly = if k == Kind::Potentiometer { y0 - 2.5 * u } else { y0 - 1.0 * u };
        let pl = pal();
        p.text(pos2(center_x, ly), Align2::CENTER_BOTTOM, &part.comp.refdes, FontId::proportional((0.55 * u).clamp(6.0, 18.0)), Color32::from_rgb(90, 86, 80).linear_multiply(if pl.dark { 1.0 } else { 1.0 }));
        let _ = x0;
    }
}

fn draw_flying_body(p: &egui::Painter, part: &BoardPart, _hs: &dyn Fn(BH) -> Pos2, ws: &dyn Fn(f32, f32) -> Pos2, u: f32, live: &Live, selected: bool) {
    let Some((bx, by)) = part.body else {
        return;
    };
    let (w, h) = flying_size(part.comp.kind);
    let c = ws(bx, by);
    let r = Rect::from_center_size(c, vec2(w * u, h * u));
    let prm = &part.comp.params;
    let pl = pal();
    p.rect_filled(r.translate(vec2(0.0, 0.25 * u)), 0.4 * u, Color32::from_black_alpha(if pl.dark { 110 } else { 50 }));
    let fsz = |s: f32| FontId::proportional((s * u).clamp(6.0, 30.0));
    match part.comp.kind {
        Kind::DcVoltage => {
            p.rect_filled(r, 0.3 * u, Color32::from_rgb(30, 32, 36));
            let top = Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + h * u * 0.35));
            p.rect_filled(top, 0.3 * u, Color32::from_rgb(200, 140, 40));
            p.text(r.center() + vec2(0.0, 0.35 * u), Align2::CENTER_CENTER, fmt_si(prm.value, "V"), fsz(0.75), Color32::WHITE);
            p.text(top.center(), Align2::CENTER_CENTER, "+   −", fsz(0.55), Color32::from_rgb(40, 30, 10));
        }
        Kind::ClockSource => {
            p.rect_filled(r, 0.35 * u, Color32::from_rgb(214, 218, 226));
            p.rect_stroke(r, 0.35 * u, Stroke::new(1.0, Color32::from_rgb(150, 155, 165)));
            let scr = Rect::from_min_max(r.min + vec2(0.3 * u, 0.3 * u), pos2(r.max.x - 0.3 * u, r.center().y + 0.35 * u));
            p.rect_filled(scr, 0.15 * u, Color32::from_rgb(16, 24, 26));
            let ph = live.time * prm.freq.min(4.0) as f32;
            let mut pts = Vec::new();
            for i in 0..=40 {
                let t = i as f32 / 40.0;
                let v = if ((t * 2.0 + ph).fract()) < (prm.duty as f32 / 100.0) { 1.0 } else { -1.0 };
                pts.push(pos2(scr.left() + 0.2 * u + t * (scr.width() - 0.4 * u), scr.center().y - v * scr.height() * 0.28));
            }
            p.add(Shape::line(pts, Stroke::new(1.5, Color32::from_rgb(90, 255, 140))));
            p.text(pos2(r.center().x, r.bottom() - 0.5 * u), Align2::CENTER_CENTER, fmt_si(prm.freq, "Hz"), fsz(0.55), Color32::from_rgb(40, 40, 50));
        }
        Kind::Motor => {
            p.circle_filled(c, 1.5 * u, Color32::from_rgb(170, 175, 185));
            p.circle_stroke(c, 1.5 * u, Stroke::new(1.0, Color32::from_rgb(110, 115, 125)));
            p.circle_filled(c, 0.9 * u, Color32::from_rgb(200, 205, 212));
            for k in 0..3 {
                let a = live.spin + k as f32 * std::f32::consts::TAU / 3.0;
                p.line_segment([c, c + vec2(a.cos(), a.sin()) * 0.85 * u], Stroke::new(0.25 * u, Color32::from_rgb(230, 120, 40)));
            }
            p.circle_filled(c, 0.22 * u, Color32::from_rgb(60, 60, 66));
        }
        Kind::Multimeter => {
            p.rect_filled(r, 0.5 * u, Color32::from_rgb(245, 196, 40));
            let inner = r.shrink(0.25 * u);
            p.rect_filled(inner, 0.4 * u, Color32::from_rgb(40, 40, 46));
            let lcd = Rect::from_min_size(inner.min + vec2(0.25 * u, 0.3 * u), vec2(inner.width() - 0.5 * u, 1.3 * u));
            p.rect_filled(lcd, 0.15 * u, Color32::from_rgb(196, 214, 170));
            let txt = live.reading.clone().unwrap_or_else(|| "----".into());
            p.text(lcd.center(), Align2::CENTER_CENTER, txt, FontId::monospace((0.62 * u).clamp(6.0, 26.0)), Color32::from_rgb(20, 30, 20));
            let dial = pos2(inner.center().x, lcd.bottom() + 1.4 * u);
            p.circle_filled(dial, 0.9 * u, Color32::from_rgb(70, 70, 78));
            let a = match prm.meter {
                MeterMode::Volts => -0.6,
                MeterMode::Amps => 0.6,
                MeterMode::Ohms => 1.6,
                MeterMode::Decibel => -1.6,
            } - std::f32::consts::FRAC_PI_2;
            p.line_segment([dial, dial + vec2(a.cos(), a.sin()) * 0.8 * u], Stroke::new(0.18 * u, Color32::WHITE));
            let mode = match prm.meter {
                MeterMode::Volts => "V",
                MeterMode::Amps => "A",
                MeterMode::Ohms => "Ω",
                MeterMode::Decibel => "dB",
            };
            p.text(pos2(inner.center().x, inner.bottom() - 0.45 * u), Align2::CENTER_CENTER, format!("{} {}", mode, if prm.ac { "AC" } else { "DC" }), fsz(0.5), Color32::from_rgb(220, 220, 225));
        }
        _ => {
            p.rect_filled(r, 0.3 * u, pl.box_fill);
        }
    }
    p.text(pos2(r.center().x, r.top() - 0.15 * u), Align2::CENTER_BOTTOM, &part.comp.refdes, fsz(0.55), pl.label);
    if selected {
        p.rect_stroke(r.expand(4.0), 0.5 * u, Stroke::new(2.0, pl.select));
    }
}
