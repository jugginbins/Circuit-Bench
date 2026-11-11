//! Helper functions for the main window.

use super::*;

pub(super) const PART_GROUPS: &[(&str, &[Kind])] = &[
    ("Sources", &[Kind::DcVoltage, Kind::AcVoltage, Kind::ClockSource, Kind::DcCurrent, Kind::AcCurrent, Kind::Ground]),
    ("Basic", &[Kind::Resistor, Kind::Capacitor, Kind::Inductor, Kind::Potentiometer, Kind::Fuse]),
    ("Switches", &[Kind::Switch, Kind::PushButton, Kind::SpdtSwitch]),
    ("Diodes", &[Kind::Diode, Kind::Zener, Kind::Led]),
    ("Analog", &[Kind::OpAmp]),
    ("Outputs", &[Kind::Lamp, Kind::Motor, Kind::Buzzer]),
    ("Indicators", &[Kind::Voltmeter, Kind::Ammeter, Kind::Probe]),
];

pub(super) const INSTRUMENTS: [Kind; 6] = [Kind::Multimeter, Kind::FuncGen, Kind::Oscilloscope, Kind::Wattmeter, Kind::BodePlotter, Kind::FreqCounter];

/// Extra meter slots for instruments that track more than one quantity.
pub(super) fn sub_id(id: u64, k: u64) -> u64 {
    id | (k << 56)
}

fn group_tint(kind: Kind) -> Color32 {
    match kind {
        Kind::DcVoltage | Kind::AcVoltage | Kind::ClockSource | Kind::DcCurrent | Kind::AcCurrent | Kind::Ground => Color32::from_rgb(240, 90, 80),
        Kind::Resistor | Kind::Capacitor | Kind::Inductor | Kind::Potentiometer | Kind::Fuse => Color32::from_rgb(70, 140, 250),
        Kind::Switch | Kind::PushButton | Kind::SpdtSwitch => Color32::from_rgb(250, 170, 40),
        Kind::Diode | Kind::Zener | Kind::Led => Color32::from_rgb(170, 90, 240),
        Kind::OpAmp => Color32::from_rgb(40, 190, 170),
        Kind::Lamp | Kind::Motor | Kind::Buzzer => Color32::from_rgb(250, 200, 40),
        _ => Color32::from_rgb(60, 190, 100),
    }
}

pub(super) fn log_freq_fmt(m: egui_plot::GridMark, _: usize, _: &std::ops::RangeInclusive<f64>) -> String {
    if (m.value - m.value.round()).abs() < 1e-6 {
        fmt_si(10f64.powf(m.value), "Hz")
    } else {
        String::new()
    }
}

pub(super) fn part_row(ui: &mut Ui, kind: Kind, selected: bool) -> egui::Response {
    let pl = pal();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hov"), resp.hovered() || selected, 0.15);
    let p = ui.painter_at(rect.expand(2.0));
    let tint = group_tint(kind);
    if hover > 0.0 {
        let base = if selected { pl.accent } else { pl.widget };
        let a = if selected { 0.35 + 0.2 * hover } else { hover };
        p.rect_filled(rect, 7.0, theme::blend(pl.panel, base, a.min(1.0)));
    }
    if selected {
        p.rect_stroke(rect, 7.0, Stroke::new(1.0, pl.accent));
    }
    let tile = Rect::from_min_size(rect.min + vec2(4.0 + hover * 2.0, 3.0), vec2(44.0, 24.0));
    p.rect_filled(tile, 6.0, theme::blend(pl.box_fill, tint, 0.18));
    p.rect_filled(Rect::from_min_size(tile.min, vec2(3.0, tile.height())), 2.0, tint);
    draw::draw_icon(&p, tile.shrink2(vec2(4.0, 2.0)), kind, pl.ink);
    p.text(pos2(tile.right() + 9.0, rect.center().y), Align2::LEFT_CENTER, kind.name(), FontId::proportional(13.5), pl.text);
    resp.on_hover_text(format!("Place a {}", kind.name().to_lowercase()))
}

#[derive(Clone, Copy)]
pub(super) enum SimIcon {
    Run,
    Pause,
    Stop,
}

pub(super) fn sim_button(ui: &mut Ui, icon: SimIcon, active: bool, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 26.0), Sense::click());
    let p = ui.painter_at(rect);
    let base = match icon {
        SimIcon::Run => Color32::from_rgb(30, 160, 60),
        SimIcon::Pause => Color32::from_rgb(220, 160, 0),
        SimIcon::Stop => Color32::from_rgb(200, 40, 40),
    };
    let fill = if active {
        base
    } else if resp.hovered() {
        Color32::from_gray(215)
    } else {
        Color32::from_gray(228)
    };
    p.rect(rect.shrink(1.0), 4.0, fill, Stroke::new(1.0, Color32::from_gray(160)));
    let fg = if active { Color32::WHITE } else { base };
    let c = rect.center();
    match icon {
        SimIcon::Run => {
            p.add(Shape::convex_polygon(vec![c + vec2(-5.0, -7.0), c + vec2(7.0, 0.0), c + vec2(-5.0, 7.0)], fg, Stroke::NONE));
        }
        SimIcon::Pause => {
            p.rect_filled(Rect::from_center_size(c + vec2(-3.5, 0.0), vec2(4.0, 13.0)), 1.0, fg);
            p.rect_filled(Rect::from_center_size(c + vec2(3.5, 0.0), vec2(4.0, 13.0)), 1.0, fg);
        }
        SimIcon::Stop => {
            p.rect_filled(Rect::from_center_size(c, vec2(12.0, 12.0)), 1.0, fg);
        }
    }
    resp.on_hover_text(tip)
}

pub(super) fn key_field(ui: &mut Ui, key: &mut char) {
    let mut s = if *key == ' ' { "Space".to_string() } else { key.to_string() };
    let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(60.0));
    if r.changed() {
        let t = s.trim();
        if t.eq_ignore_ascii_case("space") || t.is_empty() {
            *key = ' ';
        } else if let Some(c) = t.chars().last() {
            if c.is_ascii_alphanumeric() {
                *key = c.to_ascii_uppercase();
            }
        }
    }
}

pub(super) fn natural_key(s: &str) -> (String, u32) {
    let p: String = s.chars().take_while(|c| !c.is_ascii_digit()).collect();
    let n: u32 = s[p.len()..].parse().unwrap_or(0);
    (p, n)
}

pub(super) fn palette(k: usize) -> Color32 {
    const P: [Color32; 8] = [
        Color32::from_rgb(220, 40, 40),
        Color32::from_rgb(30, 90, 220),
        Color32::from_rgb(20, 150, 60),
        Color32::from_rgb(230, 130, 0),
        Color32::from_rgb(140, 50, 180),
        Color32::from_rgb(0, 150, 160),
        Color32::from_rgb(120, 80, 40),
        Color32::from_rgb(90, 90, 90),
    ];
    P[k % P.len()]
}

pub(super) fn seg_dist(p: (f32, f32), a: GP, b: GP) -> f32 {
    let (ax, ay, bx, by) = (a.x as f32, a.y as f32, b.x as f32, b.y as f32);
    let (dx, dy) = (bx - ax, by - ay);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p.0 - ax) * dx + (p.1 - ay) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cy) = (ax + t * dx, ay + t * dy);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

pub(super) fn dashed_rect(p: &egui::Painter, r: Rect, col: Color32) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    for w in pts.windows(2) {
        p.extend(Shape::dashed_line(&[w[0], w[1]], Stroke::new(1.2, col), 4.0, 3.0));
    }
}

/// Points where three or more conductors meet.
pub(super) fn junctions(c: &Circuit) -> Vec<GP> {
    let mut pins: HashMap<GP, usize> = HashMap::new();
    for comp in &c.components {
        for p in comp.pin_positions() {
            *pins.entry(p).or_default() += 1;
        }
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for w in &c.wires {
        for e in [w.points[0], *w.points.last().unwrap()] {
            if !seen.insert(e) {
                continue;
            }
            let mut deg = pins.get(&e).copied().unwrap_or(0);
            for w2 in &c.wires {
                let n = w2.points.len();
                if w2.points[0] == e {
                    deg += 1;
                }
                if n > 1 && w2.points[n - 1] == e {
                    deg += 1;
                }
                for k in 1..n.saturating_sub(1) {
                    if w2.points[k] == e {
                        deg += 2;
                    }
                }
                for k in 1..n {
                    let (a, b) = (w2.points[k - 1], w2.points[k]);
                    if a != e && b != e && sim::on_segment(e, a, b) {
                        deg += 2;
                    }
                }
            }
            if deg >= 3 {
                out.push(e);
            }
        }
    }
    out
}

/// Keep wires attached to moved or rotated parts.
pub(super) fn reroute(orig: &Circuit, c: &mut Circuit, changed: &HashSet<u64>, translate: Option<GP>, moved_wires: &HashSet<u64>) {
    let mut map: HashMap<GP, GP> = HashMap::new();
    for oc in &orig.components {
        if !changed.contains(&oc.id) {
            continue;
        }
        if let Some(nc) = c.comp(oc.id) {
            for (a, b) in oc.pin_positions().into_iter().zip(nc.pin_positions()) {
                map.insert(a, b);
            }
        }
    }
    for w in c.wires.iter_mut() {
        if moved_wires.contains(&w.id) {
            if let Some(d) = translate {
                for p in w.points.iter_mut() {
                    *p = p.add(d);
                }
            }
            continue;
        }
        let n = w.points.len();
        if n < 2 {
            continue;
        }
        let f = map.get(&w.points[0]).copied();
        let l = map.get(&w.points[n - 1]).copied();
        match (f, l, translate) {
            (Some(_), Some(_), Some(d)) => {
                for p in w.points.iter_mut() {
                    *p = p.add(d);
                }
            }
            _ => {
                if let Some(nf) = f {
                    reroute_end(&mut w.points, nf);
                }
                if let Some(nl) = l {
                    w.points.reverse();
                    reroute_end(&mut w.points, nl);
                    w.points.reverse();
                }
            }
        }
        w.points = simplify(std::mem::take(&mut w.points));
    }
    c.wires.retain(|w| w.points.len() >= 2);
}

fn reroute_end(pts: &mut Vec<GP>, new: GP) {
    let p0 = pts[0];
    let p1 = pts[1];
    if p0 == new {
        return;
    }
    let horizontal = p0.y == p1.y;
    pts[0] = new;
    if new.x == p1.x || new.y == p1.y {
        return;
    }
    if pts.len() == 2 {
        if horizontal {
            let mx = (new.x + p1.x) / 2;
            pts.splice(1..1, [GP::new(mx, new.y), GP::new(mx, p1.y)]);
        } else {
            let my = (new.y + p1.y) / 2;
            pts.splice(1..1, [GP::new(new.x, my), GP::new(p1.x, my)]);
        }
    } else {
        let elbow = if horizontal { GP::new(p1.x, new.y) } else { GP::new(new.x, p1.y) };
        pts.insert(1, elbow);
    }
}

/// Measure every metering part after a time step.
pub(super) fn sample(c: &Circuit, sim: &Sim, meters: &mut HashMap<u64, Meter>, scopes: &mut HashMap<u64, Scope>, window: f64) {
    let t = sim.t;
    for (ci, comp) in c.components.iter().enumerate() {
        let nets = &sim.nl.comp_nets[ci];
        let val = match comp.kind {
            Kind::Voltmeter => Some(sim.v(nets[0]) - sim.v(nets[1])),
            Kind::Ammeter => Some(sim.comp_current(ci)),
            Kind::Probe => Some(sim.v(nets[0])),
            Kind::Led | Kind::Fuse | Kind::Motor | Kind::Buzzer | Kind::PushButton => Some(sim.comp_current(ci)),
            Kind::FreqCounter => Some(sim.v(nets[0])),
            Kind::Wattmeter => {
                let v = sim.v(nets[0]) - sim.v(nets[1]);
                let i = sim.comp_current(ci);
                for (k, val) in [(1u64, v), (2u64, i)] {
                    let m = meters.entry(sub_id(comp.id, k)).or_default();
                    m.window = window;
                    m.push(t, val);
                }
                Some(v * i)
            }
            Kind::Lamp => Some((sim.v(nets[0]) - sim.v(nets[1])) * sim.comp_current(ci)),
            Kind::Multimeter => Some(match comp.params.meter {
                MeterMode::Amps => sim.comp_current(ci),
                MeterMode::Ohms => (sim.v(nets[0]) - sim.v(nets[1])) / if comp.params.i_test > 0.0 { comp.params.i_test } else { sim::OHM_TEST_I },
                _ => sim.v(nets[0]) - sim.v(nets[1]),
            }),
            Kind::Oscilloscope => {
                let g = if sim.nl.net_pin_count[nets[3]] > 1 { sim.v(nets[3]) } else { 0.0 };
                let sc = scopes.entry(comp.id).or_default();
                sc.push(t, sim.v(nets[0]) - g, sim.v(nets[1]) - g, sim.v(nets[2]) - g);
                None
            }
            _ => None,
        };
        if let Some(v) = val {
            let m = meters.entry(comp.id).or_default();
            m.window = window;
            m.push(t, v);
        }
    }
}

pub(super) fn multimeter_text(c: &Component, m: &Meter) -> String {
    let p = &c.params;
    match p.meter {
        MeterMode::Volts => fmt_reading(if p.ac { m.ac_rms() } else { m.mean() }, "V"),
        MeterMode::Amps => fmt_reading(if p.ac { m.ac_rms() } else { m.mean() }, "A"),
        MeterMode::Ohms => {
            let r = m.mean();
            if !(0.0..=2e8).contains(&r) {
                "OL Ω".into()
            } else {
                fmt_reading(r, "Ω")
            }
        }
        MeterMode::Decibel => {
            let v = if p.ac { m.ac_rms() } else { m.mean().abs() };
            format!("{:.3} dB", 20.0 * (v.max(1e-12) / 0.7746).log10())
        }
    }
}

pub(super) fn scope_colors(c: &Circuit, idx: usize) -> [Color32; 2] {
    let pins = c.components[idx].pin_positions();
    let mut out = [Color32::from_rgb(255, 80, 60), Color32::from_rgb(70, 160, 255)];
    for k in 0..2 {
        if let Some(w) = c.wires.iter().find(|w| w.points[0] == pins[k] || *w.points.last().unwrap() == pins[k]) {
            if w.color != WireColor::Red || k == 0 {
                let [r, g, b] = w.color.rgb();
                out[k] = if w.color == WireColor::Black { Color32::from_gray(230) } else { brighten(Color32::from_rgb(r, g, b)) };
            }
        }
    }
    out
}

fn brighten(c: Color32) -> Color32 {
    let f = |v: u8| ((v as f32 * 0.6) + 100.0).min(255.0) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

pub(super) fn erc(c: &Circuit) -> Vec<String> {
    let mut out = Vec::new();
    let nl = sim::build_netlist(c);
    if !nl.has_ground && !c.components.is_empty() {
        out.push("No ground: place a Ground part and connect it.".into());
    }
    for (ci, comp) in c.components.iter().enumerate() {
        let optional = |k: usize| match comp.kind {
            Kind::Oscilloscope => k >= 1,
            Kind::FuncGen => k == 2,
            Kind::Potentiometer => k == 2,
            Kind::SpdtSwitch => k >= 1,
            _ => false,
        };
        for (k, n) in nl.comp_nets[ci].iter().enumerate() {
            if nl.net_pin_count[*n] == 1 && *n != 0 && !optional(k) {
                out.push(format!("{} pin {} is not connected.", comp.refdes, comp.kind.pins()[k].2));
            }
        }
        let nets = &nl.comp_nets[ci];
        if nets.len() == 2 && nets[0] == nets[1] && !matches!(comp.kind, Kind::Ground | Kind::Probe) {
            out.push(format!("{} is shorted (both pins on the same net).", comp.refdes));
        }
    }
    for (wi, n) in nl.wire_nets.iter().enumerate() {
        if *n == usize::MAX {
            out.push(format!("Wire #{} is not connected to any part.", wi + 1));
        }
    }
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for comp in &c.components {
        if !comp.refdes.is_empty() {
            *seen.entry(&comp.refdes).or_default() += 1;
        }
    }
    for (k, v) in seen {
        if v > 1 {
            out.push(format!("RefDes {} is used {} times.", k, v));
        }
    }
    out
}

pub(super) fn spice_netlist(c: &Circuit) -> String {
    let nl = sim::build_netlist(c);
    let mut s = format!("* {}\n* exported by Circuit Bench\n", if c.title.is_empty() { "Untitled" } else { &c.title });
    let mut need_d = false;
    let mut models = Vec::new();
    for (ci, comp) in c.components.iter().enumerate() {
        let n = &nl.comp_nets[ci];
        let p = &comp.params;
        let name = comp.refdes.replace(' ', "_");
        match comp.kind {
            Kind::Resistor => s += &format!("R{} {} {} {}\n", name.trim_start_matches('R'), n[0], n[1], p.value),
            Kind::Capacitor => s += &format!("C{} {} {} {} IC={}\n", name.trim_start_matches('C'), n[0], n[1], p.value, p.ic),
            Kind::Inductor => s += &format!("L{} {} {} {}\n", name.trim_start_matches('L'), n[0], n[1], p.value),
            Kind::Potentiometer => {
                let r = p.value * p.setting / 100.0;
                s += &format!("R{}a {} {} {}\nR{}b {} {} {}\n", name, n[0], n[2], r.max(1e-3), name, n[2], n[1], (p.value - r).max(1e-3));
            }
            Kind::DcVoltage => s += &format!("V{} {} {} DC {}\n", name.trim_start_matches('V'), n[0], n[1], p.value),
            Kind::DcCurrent => s += &format!("I{} {} {} DC {}\n", name.trim_start_matches('I'), n[1], n[0], p.value),
            Kind::AcVoltage => s += &format!("V{} {} {} SIN({} {} {} 0 0 {}) AC {}\n", name.trim_start_matches('V'), n[0], n[1], p.offset, p.value, p.freq, p.phase, p.value),
            Kind::Diode => {
                need_d = true;
                s += &format!("D{} {} {} DGEN\n", name.trim_start_matches('D'), n[0], n[1]);
            }
            Kind::Zener => {
                models.push(format!(".model DZ{} D(IS={} N={} BV={} IBV=1m)", name, p.is, p.n, p.value));
                s += &format!("D{} {} {} DZ{}\n", name.trim_start_matches('D'), n[0], n[1], name);
            }
            Kind::Led => {
                models.push(format!(".model DLED{} D(IS={:e} N=2)", name, 0.01 / (p.led.forward_voltage() / (2.0 * 0.025852)).exp()));
                s += &format!("D{} {} {} DLED{}\n", name, n[0], n[1], name);
            }
            Kind::Switch => s += &format!("R{}sw {} {} {}\n", name, n[0], n[1], if p.closed { 1e-3 } else { 1e10 }),
            Kind::Lamp => s += &format!("R{}lamp {} {} {}\n", name, n[0], n[1], p.value * p.value / p.power.max(1e-9)),
            Kind::Ammeter => s += &format!("V{}amm {} {} DC 0\n", name, n[0], n[1]),
            Kind::Voltmeter => s += &format!("R{}vm {} {} 10Meg\n", name, n[0], n[1]),
            Kind::FuncGen => {
                let wave = match p.wave {
                    Wave::Sine => format!("SIN({} {} {})", p.offset, p.value, p.freq),
                    _ => {
                        let per = 1.0 / p.freq.max(1e-12);
                        let on = per * p.duty / 100.0;
                        format!("PULSE({} {} 0 {} {} {} {})", p.offset - p.value, p.offset + p.value, per / 1000.0, per / 1000.0, on, per)
                    }
                };
                s += &format!("V{}p {} {} {} AC {}\n", name, n[0], n[1], wave, p.value);
            }
            Kind::ClockSource => {
                let per = 1.0 / p.freq.max(1e-12);
                s += &format!("V{} {} {} PULSE({} {} 0 {} {} {} {})\n", name.trim_start_matches('V'), n[0], n[1], p.offset, p.value, per / 1000.0, per / 1000.0, per * p.duty / 100.0, per);
            }
            Kind::AcCurrent => s += &format!("I{} {} {} SIN({} {} {}) AC {}\n", name.trim_start_matches('I'), n[1], n[0], p.offset, p.value, p.freq, p.value),
            Kind::PushButton => s += &format!("R{}pb {} {} {}\n", name, n[0], n[1], if p.closed != p.normally_closed { 1e-3 } else { 1e10 }),
            Kind::SpdtSwitch => {
                s += &format!("R{}a {} {} {}\n", name, n[0], n[1], if !p.closed { 1e-3 } else { 1e10 });
                s += &format!("R{}b {} {} {}\n", name, n[0], n[2], if p.closed { 1e-3 } else { 1e10 });
            }
            Kind::Fuse => s += &format!("R{}fuse {} {} {}\n", name, n[0], n[1], if p.blown { 1e10 } else { 1e-2 }),
            Kind::Motor | Kind::Buzzer => s += &format!("R{}load {} {} {}\n", name, n[0], n[1], p.value),
            Kind::OpAmp => s += &format!("E{} {} 0 {} {} {}\n", name, n[2], n[1], n[0], p.value),
            Kind::Wattmeter => s += &format!("V{}wm {} {} DC 0\n", name, n[2], n[3]),
            Kind::Multimeter | Kind::Oscilloscope | Kind::BodePlotter | Kind::FreqCounter => s += &format!("* {} ({}) is a measuring instrument\n", comp.refdes, comp.kind.name()),
            Kind::Ground | Kind::Probe => {}
        }
    }
    if need_d {
        s += ".model DGEN D(IS=2.52n N=1.752)\n";
    }
    for m in models {
        s += &m;
        s += "\n";
    }
    s += ".tran 10u 10m\n.end\n";
    s
}

pub(super) fn graph_csv(g: &GraphTab) -> String {
    match g {
        GraphTab::Op { rows, .. } => {
            let mut s = String::from("variable,value\n");
            for (n, v, _) in rows {
                s += &format!("{},{}\n", n, v);
            }
            s
        }
        GraphTab::Xy { xlabel, series, .. } => {
            let mut s = xlabel.to_string();
            for se in series {
                s += &format!(",{}", se.name);
            }
            s += "\n";
            let n = series.first().map(|x| x.pts.len()).unwrap_or(0);
            for i in 0..n {
                s += &format!("{}", series[0].pts[i][0]);
                for se in series {
                    s += &format!(",{}", se.pts.get(i).map(|p| p[1]).unwrap_or(f64::NAN));
                }
                s += "\n";
            }
            s
        }
        GraphTab::Ac { series, .. } => {
            let mut s = String::from("frequency");
            for se in series {
                s += &format!(",{} dB,{} phase", se.name, se.name);
            }
            s += "\n";
            let n = series.first().map(|x| x.mag.len()).unwrap_or(0);
            for i in 0..n {
                s += &format!("{}", 10f64.powf(series[0].mag[i][0]));
                for se in series {
                    s += &format!(",{},{}", se.mag[i][1], se.phase[i][1]);
                }
                s += "\n";
            }
            s
        }
    }
}

pub(super) fn save_png(path: &Path, img: &egui::ColorImage) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let w = std::io::BufWriter::new(file);
    let mut enc = png::Encoder::new(w, img.size[0] as u32, img.size[1] as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    let mut data = Vec::with_capacity(img.pixels.len() * 4);
    for p in &img.pixels {
        let [r, g, b, _] = p.to_srgba_unmultiplied();
        data.extend_from_slice(&[r, g, b, 255]);
    }
    writer.write_image_data(&data).map_err(|e| e.to_string())?;
    Ok(())
}

/// A theme entry with colour swatches.
pub(super) fn theme_choice(ui: &mut Ui, t: ThemeKind, selected: bool) -> egui::Response {
    let pl = t.palette();
    let (rect, resp) = ui.allocate_exact_size(vec2(170.0, 26.0), Sense::click());
    let p = ui.painter_at(rect);
    let cur = pal();
    if selected || resp.hovered() {
        p.rect_filled(rect, 6.0, theme::blend(cur.window, cur.accent, if selected { 0.35 } else { 0.18 }));
    }
    let sw = Rect::from_min_size(rect.min + vec2(6.0, 4.0), vec2(50.0, 18.0));
    p.rect_filled(sw, 5.0, pl.sheet);
    p.rect_stroke(sw, 5.0, Stroke::new(1.0, cur.grid));
    for (k, c) in [pl.ink, pl.accent, pl.flow].iter().enumerate() {
        p.circle_filled(sw.left_center() + vec2(10.0 + 15.0 * k as f32, 0.0), 5.0, *c);
    }
    p.text(pos2(sw.right() + 10.0, rect.center().y), Align2::LEFT_CENTER, t.name(), FontId::proportional(14.0), cur.text);
    resp
}

/// Sun / moon button for quick dark-mode switching.
pub(super) fn mode_toggle(ui: &mut Ui, dark: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 26.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, dark, 0.25);
    let pl = pal();
    let p = ui.painter_at(rect);
    p.rect(rect.shrink(1.0), 6.0, if resp.hovered() { pl.widget } else { pl.panel }, Stroke::new(1.0, pl.grid));
    let c = rect.center();
    let col = theme::blend(Color32::from_rgb(250, 180, 30), Color32::from_rgb(200, 210, 255), t);
    p.circle_filled(c, 6.5, col);
    // the moon's bite slides in as dark mode turns on
    if t > 0.01 {
        p.circle_filled(c + vec2(4.0 + 6.0 * (1.0 - t), -3.0), 5.5 * t, if resp.hovered() { pl.widget } else { pl.panel });
    }
    if t < 0.99 {
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::TAU / 8.0;
            let d = vec2(a.cos(), a.sin());
            p.line_segment([c + d * 8.5, c + d * 11.0], Stroke::new(1.5, col.linear_multiply(1.0 - t)));
        }
    }
    resp
}
