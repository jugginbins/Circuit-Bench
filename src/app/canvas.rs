//! The schematic sheet: drawing, hit testing, placing, wiring and moving parts.

use super::util::*;
use super::*;

impl App {
    pub(super) fn fit_view(&mut self, rect: Rect) {
        let mut lo = (f32::MAX, f32::MAX);
        let mut hi = (f32::MIN, f32::MIN);
        let mut any = false;
        for c in &self.circuit.components {
            let (x0, y0, x1, y1) = c.kind.bounds();
            for (x, y) in [(x0, y0 - 2.5), (x1, y1 + 2.5), (x0, y1 + 2.5), (x1, y0 - 2.5)] {
                let (rx, ry) = rotate_f(x, y, c.rot);
                let (wx, wy) = (c.pos.x as f32 + rx, c.pos.y as f32 + ry);
                lo = (lo.0.min(wx), lo.1.min(wy));
                hi = (hi.0.max(wx), hi.1.max(wy));
                any = true;
            }
        }
        for w in &self.circuit.wires {
            for p in &w.points {
                lo = (lo.0.min(p.x as f32), lo.1.min(p.y as f32));
                hi = (hi.0.max(p.x as f32), hi.1.max(p.y as f32));
                any = true;
            }
        }
        let (z, pan) = if !any {
            (1.6, rect.center().to_vec2() - self.canvas_min)
        } else {
            let w = (hi.0 - lo.0 + 8.0) * GRID;
            let h = (hi.1 - lo.1 + 8.0) * GRID;
            let z = (rect.width() / w).min(rect.height() / h).clamp(0.4, 2.2);
            let s = GRID * z;
            let cx = (lo.0 + hi.0) / 2.0;
            let cy = (lo.1 + hi.1) / 2.0;
            (z, (rect.center().to_vec2() - self.canvas_min) - vec2(cx * s, cy * s))
        };
        self.zoom_target = z;
        self.zoom_anchor = None;
        if self.fit_snap || !self.smooth_zoom {
            self.zoom = z;
            self.pan = pan;
            self.pan_target = None;
            self.fit_snap = false;
        } else {
            self.pan_target = Some(pan);
        }
    }

    /// Ease zoom and pan towards their targets.
    fn animate_view(&mut self, rect: Rect, dt: f32) {
        let k = if self.smooth_zoom { 1.0 - (-dt * 14.0).exp() } else { 1.0 };
        if let Some(pt) = self.pan_target {
            self.zoom += (self.zoom_target - self.zoom) * k;
            self.pan += (pt - self.pan) * k;
            if (pt - self.pan).length() < 0.5 && (self.zoom_target - self.zoom).abs() < 1e-3 {
                self.pan = pt;
                self.zoom = self.zoom_target;
                self.pan_target = None;
            }
            return;
        }
        if (self.zoom_target - self.zoom).abs() > 1e-4 {
            let (m, world) = self.zoom_anchor.unwrap_or_else(|| {
                let m = rect.center();
                (m, (m - rect.min - self.pan) / (GRID * self.zoom))
            });
            self.zoom_anchor = Some((m, world));
            self.zoom += (self.zoom_target - self.zoom) * k;
            if (self.zoom_target - self.zoom).abs() < 1e-4 {
                self.zoom = self.zoom_target;
            }
            self.pan = m - rect.min - world * (GRID * self.zoom);
        } else {
            self.zoom = self.zoom_target;
            self.zoom_anchor = None;
        }
    }

    pub(super) fn comp_screen_rect(c: &Component, origin: Pos2, s: f32) -> Rect {
        let (x0, y0, x1, y1) = c.kind.bounds();
        let mut r = Rect::NOTHING;
        for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y1)] {
            let (rx, ry) = rotate_f(x, y, c.rot);
            r.extend_with(origin + vec2((c.pos.x as f32 + rx) * s, (c.pos.y as f32 + ry) * s));
        }
        r
    }

    pub(super) fn canvas(&mut self, ui: &mut Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        self.canvas_min = rect.min.to_vec2();
        if self.need_fit {
            let mut fit_rect = rect;
            if !self.open_inst.is_empty() {
                // leave room for the instrument panel that opens on the right
                fit_rect.max.x = (rect.right() - 560.0).max(rect.left() + rect.width() * 0.45);
            }
            self.fit_view(fit_rect);
            self.need_fit = false;
        }
        // zoom / pan with wheel and trackpad (Ctrl/Cmd + wheel or pinch zooms)
        if resp.hovered() {
            let (scroll, zd, hover) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
            if zd != 1.0 {
                if let Some(m) = hover {
                    let world = (m - rect.min - self.pan) / (GRID * self.zoom);
                    self.pan_target = None;
                    self.zoom_anchor = Some((m, world));
                    self.zoom_target = (self.zoom_target * zd).clamp(0.25, 8.0);
                }
            } else if scroll != egui::Vec2::ZERO {
                self.pan += scroll;
                if let Some(pt) = self.pan_target.as_mut() {
                    *pt += scroll;
                }
                if let Some((m, _)) = self.zoom_anchor {
                    self.zoom_anchor = Some((m, (m - rect.min - self.pan) / (GRID * self.zoom)));
                }
            }
        }
        let dt = ui.input(|i| i.stable_dt).clamp(0.001, 0.05);
        self.animate_view(rect, dt);
        if resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Secondary) {
            self.pan += resp.drag_delta();
        }
        let s = GRID * self.zoom;
        let origin = rect.min + self.pan;
        let to_screen = |p: GP| origin + vec2(p.x as f32 * s, p.y as f32 * s);
        let to_world = |q: Pos2| ((q.x - origin.x) / s, (q.y - origin.y) / s);

        // sheet & grid
        let pl = pal();
        painter.rect_filled(rect, 0.0, pl.sheet);
        if self.show_grid && s >= 4.0 {
            let step = if s < 7.0 { 2 } else { 1 };
            let (wx0, wy0) = to_world(rect.min);
            let (wx1, wy1) = to_world(rect.max);
            let dot = pl.grid;
            let mut gx = (wx0.floor() as i32 / step) * step;
            while gx as f32 <= wx1 {
                let mut gy = (wy0.floor() as i32 / step) * step;
                while gy as f32 <= wy1 {
                    let p = to_screen(GP::new(gx, gy));
                    let major = gx % 10 == 0 && gy % 10 == 0;
                    painter.circle_filled(p, if major { 1.3 } else { 0.8 }, dot);
                    gy += step;
                }
                gx += step;
            }
        }

        // ---- hit testing
        let hover = resp.hover_pos();
        let mw = hover.map(to_world);
        let snapped = mw.map(|(x, y)| GP::new(x.round() as i32, y.round() as i32));
        let mut hover_pin: Option<GP> = None;
        let mut hover_comp: Option<u64> = None;
        let mut hover_wire: Option<u64> = None;
        if let Some((x, y)) = mw {
            let mut best = 0.65f32;
            for c in &self.circuit.components {
                for p in c.pin_positions() {
                    let d = ((p.x as f32 - x).powi(2) + (p.y as f32 - y).powi(2)).sqrt();
                    if d < best {
                        best = d;
                        hover_pin = Some(p);
                    }
                }
            }
            for c in self.circuit.components.iter().rev() {
                let (x0, y0, x1, y1) = c.kind.bounds();
                let inv = (4 - c.rot % 4) % 4;
                let (lx, ly) = rotate_f(x - c.pos.x as f32, y - c.pos.y as f32, inv);
                let m = 0.3;
                if lx >= x0 - m && lx <= x1 + m && ly >= y0 - m && ly <= y1 + m {
                    hover_comp = Some(c.id);
                    break;
                }
            }
            if hover_comp.is_none() {
                'outer: for w in self.circuit.wires.iter().rev() {
                    for k in 1..w.points.len() {
                        if seg_dist((x, y), w.points[k - 1], w.points[k]) < 0.5 {
                            hover_wire = Some(w.id);
                            break 'outer;
                        }
                    }
                }
            }
        }
        if self.tool == Tool::Select && self.wiring.is_none() && hover_pin.is_some() && matches!(self.drag, Drag::None) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }

        // ---- input
        let (pressed, released, down, shift, ptr) =
            ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_released(), i.pointer.primary_down(), i.modifiers.shift, i.pointer.interact_pos()));
        let dbl = resp.double_clicked();
        if pressed && resp.hovered() {
            if let Some(sp) = snapped {
                self.on_press(sp, hover_pin, hover_comp, hover_wire, shift, ptr.unwrap_or(rect.center()));
            }
        }
        if down {
            let comps = self.selected_comps();
            let wires = self.selected_wires();
            if let (Drag::Move { start, delta, orig }, Some(sp)) = (&mut self.drag, snapped) {
                let d = sp.sub(*start);
                if d != *delta {
                    *delta = d;
                    let mut c = orig.clone();
                    for comp in c.components.iter_mut() {
                        if comps.contains(&comp.id) {
                            comp.pos = comp.pos.add(d);
                        }
                    }
                    reroute(orig, &mut c, &comps, Some(d), &wires);
                    self.circuit = c;
                }
            }
        }
        if released {
            match std::mem::replace(&mut self.drag, Drag::None) {
                Drag::Move { delta, orig, .. } => {
                    if delta != GP::new(0, 0) {
                        self.push_undo(orig);
                        self.edit_stop();
                    }
                }
                Drag::Band { start } => {
                    if let Some(end) = ptr {
                        let r = Rect::from_two_pos(start, end);
                        if r.width() > 3.0 || r.height() > 3.0 {
                            for c in &self.circuit.components {
                                if r.contains(Self::comp_screen_rect(c, origin, s).center()) {
                                    self.selection.insert(Sel::Comp(c.id));
                                }
                            }
                            for w in &self.circuit.wires {
                                if w.points.iter().all(|p| r.contains(to_screen(*p))) {
                                    self.selection.insert(Sel::Wire(w.id));
                                }
                            }
                        }
                    }
                }
                Drag::None => {}
            }
        }
        if dbl {
            if self.wiring.is_some() {
                self.finish_wire();
            } else if let Some(id) = hover_comp {
                self.open_props(id);
            } else if let Some(id) = hover_wire {
                self.wire_props = Some(id);
            }
        }
        if resp.secondary_clicked() && self.wiring.is_some() {
            self.wiring = None;
        }

        // hover info (net voltage under the pointer while simulating)
        self.hover_info.clear();
        if let Some(sim) = self.sim.as_ref().filter(|_| !self.sim_board) {
            let net = if let Some(p) = hover_pin {
                self.circuit
                    .components
                    .iter()
                    .enumerate()
                    .find_map(|(ci, c)| c.pin_positions().iter().position(|q| *q == p).and_then(|k| sim.nl.comp_nets.get(ci).map(|v| v[k])))
            } else if let Some(wid) = hover_wire {
                self.circuit.wires.iter().position(|w| w.id == wid).and_then(|i| sim.nl.wire_nets.get(i).copied()).filter(|n| *n != usize::MAX)
            } else {
                None
            };
            if let Some(n) = net {
                self.hover_info = format!("Net {}: {}", n, fmt_reading(sim.v(n), "V"));
            }
        }

        // ---- draw wires
        let sel_w = self.selected_wires();
        let wire_w = (0.16 * s).clamp(1.0, 4.0);
        let net_of_wire: Option<&Vec<usize>> = self.sim.as_ref().filter(|_| !self.sim_board).map(|s| &s.nl.wire_nets);
        for (wi, w) in self.circuit.wires.iter().enumerate() {
            let [r, g, b] = w.color.rgb();
            let col = if sel_w.contains(&w.id) { pl.select } else { theme::wire_color([r, g, b]) };
            let pts: Vec<Pos2> = w.points.iter().map(|p| to_screen(*p)).collect();
            if sel_w.contains(&w.id) {
                painter.add(Shape::line(pts.clone(), Stroke::new(wire_w + 3.0, Color32::from_rgba_unmultiplied(0, 140, 255, 60))));
            }
            if hover_wire == Some(w.id) && self.tool == Tool::Select {
                painter.add(Shape::line(pts.clone(), Stroke::new(wire_w + 2.0, if pl.dark { Color32::from_white_alpha(30) } else { Color32::from_black_alpha(30) })));
            }
            painter.add(Shape::line(pts, Stroke::new(wire_w, col)));
            if self.show_nets && w.points.len() >= 2 {
                let label = net_of_wire.and_then(|v| v.get(wi).copied()).filter(|n| *n != usize::MAX).map(|n| n.to_string());
                if let Some(label) = label {
                    let a = to_screen(w.points[0]);
                    let b = to_screen(w.points[1]);
                    let m = pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                    painter.text(m + vec2(2.0, -2.0), Align2::LEFT_BOTTOM, label, FontId::proportional((0.9 * s).clamp(8.0, 18.0)), Color32::from_rgb(0, 120, 0));
                }
            }
        }
        for p in junctions(&self.circuit) {
            painter.circle_filled(to_screen(p), (0.3 * s).clamp(2.0, 6.0), pl.junction);
        }

        // ---- animated current flow along wires
        if self.show_flow && self.running {
            if let Some(sim) = self.sim.as_ref().filter(|_| !self.sim_board) {
                let mut pin_at: HashMap<GP, (usize, usize, u32)> = HashMap::new();
                for (ci, c) in self.circuit.components.iter().enumerate() {
                    for (k, p) in c.pin_positions().into_iter().enumerate() {
                        let e = pin_at.entry(p).or_insert((ci, k, 0));
                        e.2 += 1;
                    }
                }
                let mut ends: HashMap<GP, u32> = HashMap::new();
                for w in &self.circuit.wires {
                    *ends.entry(w.points[0]).or_default() += 1;
                    *ends.entry(*w.points.last().unwrap()).or_default() += 1;
                }
                let pin_current = |p: GP| -> Option<f64> {
                    let (ci, k, n) = *pin_at.get(&p)?;
                    if n != 1 || ends.get(&p).copied().unwrap_or(0) != 1 {
                        return None;
                    }
                    let c = &self.circuit.components[ci];
                    if c.kind.pins().len() != 2 || matches!(c.kind, Kind::Ground | Kind::Probe) {
                        return None;
                    }
                    let i = sim.comp_current(ci);
                    Some(if k == 0 { i } else { -i })
                };
                let paused = self.paused;
                for w in &self.circuit.wires {
                    let along = pin_current(w.points[0]).map(|i| -i).or_else(|| pin_current(*w.points.last().unwrap()));
                    let Some(i) = along else {
                        continue;
                    };
                    if i.abs() < 1e-6 {
                        continue;
                    }
                    let mag = ((i.abs() / 1e-6).log10() / 6.0).clamp(0.0, 1.0) as f32;
                    let speed = i.signum() as f32 * (0.6 + 4.0 * mag);
                    let ph = self.flow_phase.entry(w.id).or_insert(0.0);
                    if !paused {
                        *ph = (*ph + speed * dt).rem_euclid(2.0);
                    }
                    let pts: Vec<Pos2> = w.points.iter().map(|p| to_screen(*p)).collect();
                    let seg_len: Vec<f32> = pts.windows(2).map(|q| (q[1] - q[0]).length() / s).collect();
                    let total: f32 = seg_len.iter().sum();
                    let mut d = *ph;
                    let r = (0.17 * s).clamp(1.5, 4.0);
                    while d < total {
                        let mut acc = 0.0;
                        for (k, l) in seg_len.iter().enumerate() {
                            if d <= acc + l && *l > 0.0 {
                                let t = (d - acc) / l;
                                let q = pts[k] + (pts[k + 1] - pts[k]) * t;
                                painter.circle_filled(q, r + 1.5, Color32::from_rgba_unmultiplied(pl.flow.r(), pl.flow.g(), pl.flow.b(), 60));
                                painter.circle_filled(q, r, pl.flow);
                                break;
                            }
                            acc += l;
                        }
                        d += 2.0;
                    }
                }
            }
        }

        // ---- draw components
        let sel_c = self.selected_comps();
        let stroke_w = (0.13 * s).clamp(1.0, 3.0);
        let empty = Live::default();
        for c in &self.circuit.components {
            let selected = sel_c.contains(&c.id);
            let o = to_screen(c.pos);
            let pen = Pen { painter: &painter, origin: o, s, rot: c.rot, stroke: Stroke::new(stroke_w, if selected { pal().select } else { pal().ink }) };
            let live = self.live.get(&c.id).unwrap_or(&empty);
            draw::draw_symbol(&pen, c.kind, &c.params, live);
            draw::draw_labels(&painter, c, o, s);
            if selected {
                let r = Self::comp_screen_rect(c, origin, s).expand(3.0);
                dashed_rect(&painter, r, pal().select);
            } else if hover_comp == Some(c.id) && self.tool == Tool::Select && hover_pin.is_none() {
                let r = Self::comp_screen_rect(c, origin, s).expand(3.0);
                painter.rect_stroke(r, 2.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 140, 255, 90)));
            }
        }
        if let Some(p) = hover_pin {
            if matches!(self.drag, Drag::None) {
                painter.circle_stroke(to_screen(p), (0.45 * s).max(4.0), Stroke::new(1.5, Color32::from_rgb(230, 40, 40)));
            }
        }

        // ---- previews
        if let (Tool::Place(kind), Some(sp)) = (self.tool, snapped) {
            if resp.hovered() {
                let pen = Pen { painter: &painter, origin: to_screen(sp), s, rot: self.place_rot, stroke: Stroke::new(stroke_w, pl.ink.linear_multiply(0.5)) };
                draw::draw_symbol(&pen, kind, &kind.default_params(), &empty);
            }
        }
        if let Some(pts) = &self.wiring {
            let target = hover_pin.or(snapped).unwrap_or(*pts.last().unwrap());
            let mut all = pts.clone();
            let last = *all.last().unwrap();
            if last.x != target.x && last.y != target.y {
                all.push(GP::new(target.x, last.y));
            }
            all.push(target);
            let v: Vec<Pos2> = all.iter().map(|p| to_screen(*p)).collect();
            painter.add(Shape::line(v, Stroke::new(wire_w, Color32::from_rgba_unmultiplied(210, 30, 30, 170))));
            painter.circle_filled(to_screen(target), 3.0, Color32::from_rgb(210, 30, 30));
        }
        if let (Drag::Band { start }, Some(p)) = (&self.drag, ptr) {
            let r = Rect::from_two_pos(*start, p);
            painter.rect(r, 0.0, Color32::from_rgba_unmultiplied(0, 140, 255, 25), Stroke::new(1.0, pal().select));
        }

        // title block in the corner of the sheet
        let title = if self.circuit.title.is_empty() { "Untitled" } else { &self.circuit.title };
        let tb = Rect::from_min_size(rect.right_bottom() - vec2(240.0, 46.0), vec2(232.0, 38.0));
        painter.rect(tb, 6.0, pl.window.linear_multiply(0.95), Stroke::new(1.0, pl.grid));
        painter.text(tb.left_top() + vec2(8.0, 4.0), Align2::LEFT_TOP, title, FontId::proportional(13.0), pl.text);
        painter.text(
            tb.left_bottom() + vec2(6.0, -4.0),
            Align2::LEFT_BOTTOM,
            format!("{} parts · {} wires · zoom {:.0}%", self.circuit.components.len(), self.circuit.wires.len(), self.zoom * 100.0 / 1.6),
            FontId::proportional(11.0),
            pl.weak_text,
        );
        if self.running {
            let badge = Rect::from_min_size(rect.left_top() + vec2(8.0, 8.0), vec2(150.0, 22.0));
            let col = if self.paused { Color32::from_rgb(220, 160, 0) } else { Color32::from_rgb(30, 160, 60) };
            let pulse = 0.5 + 0.5 * (self.anim_clock * 3.0).sin();
            painter.rect_filled(badge.expand(2.0 + 2.0 * pulse), 8.0, Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), (40.0 * pulse) as u8));
            painter.rect_filled(badge, 6.0, col);
            painter.circle_filled(badge.left_center() + vec2(12.0, 0.0), 3.5, Color32::from_white_alpha((150.0 + 105.0 * pulse) as u8));
            painter.text(badge.center(), Align2::CENTER_CENTER, if self.paused { "SIMULATION PAUSED" } else { "SIMULATING" }, FontId::proportional(12.0), Color32::WHITE);
        }
    }

    fn on_press(&mut self, sp: GP, hover_pin: Option<GP>, hover_comp: Option<u64>, hover_wire: Option<u64>, shift: bool, ptr: Pos2) {
        if let Tool::Place(kind) = self.tool {
            self.edit_stop();
            self.checkpoint();
            let id = self.circuit.add(kind, sp, self.place_rot);
            self.selection.clear();
            self.selection.insert(Sel::Comp(id));
            if !shift {
                self.tool = Tool::Select;
            }
            return;
        }
        if let Some(pts) = &mut self.wiring {
            let target = hover_pin.unwrap_or(sp);
            let last = *pts.last().unwrap();
            if target == last {
                return;
            }
            if last.x != target.x && last.y != target.y {
                pts.push(GP::new(target.x, last.y));
            }
            pts.push(target);
            let start = pts[0];
            let ends_on_wire = self.circuit.wires.iter().any(|w| w.points.windows(2).any(|s| sim::on_segment(target, s[0], s[1])));
            let ends_on_pin = hover_pin.is_some() && target != start;
            if ends_on_pin || ends_on_wire {
                self.finish_wire();
            }
            return;
        }
        if self.tool == Tool::Wire {
            self.wiring = Some(vec![hover_pin.unwrap_or(sp)]);
            return;
        }
        if let Some(p) = hover_pin {
            self.wiring = Some(vec![p]);
            return;
        }
        if let Some(id) = hover_comp {
            if self.running {
                if let Some(c) = self.circuit.comp_mut(id) {
                    match c.kind {
                        Kind::Switch | Kind::SpdtSwitch => c.params.closed = !c.params.closed,
                        Kind::PushButton => self.held_button = Some(id),
                        _ => {}
                    }
                }
            }
            self.select_click(Sel::Comp(id), shift, sp);
            return;
        }
        if let Some(id) = hover_wire {
            self.select_click(Sel::Wire(id), shift, sp);
            return;
        }
        if !shift {
            self.selection.clear();
        }
        self.drag = Drag::Band { start: ptr };
    }

    fn select_click(&mut self, s: Sel, shift: bool, sp: GP) {
        if shift {
            if !self.selection.remove(&s) {
                self.selection.insert(s);
            }
        } else if !self.selection.contains(&s) {
            self.selection.clear();
            self.selection.insert(s);
        }
        self.drag = Drag::Move { start: sp, delta: GP::new(0, 0), orig: self.circuit.clone() };
    }

    pub(super) fn finish_wire(&mut self) {
        if let Some(pts) = self.wiring.take() {
            let pts = simplify(pts);
            if pts.len() >= 2 {
                self.edit_stop();
                self.checkpoint();
                self.circuit.add_wire(pts);
                self.selection.clear();
            }
        }
    }

    pub(super) fn open_props(&mut self, id: u64) {
        if let Some(c) = self.circuit.comp(id) {
            if c.kind.is_instrument() {
                if !self.open_inst.contains(&id) {
                    self.open_inst.push(id);
                }
            } else if c.kind != Kind::Ground {
                self.props = Some(id);
                self.props_orig = Some(self.circuit.clone());
            }
        }
    }
}
