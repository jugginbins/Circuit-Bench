//! Circuit simulation engine: netlist extraction and modified nodal analysis
//! (DC operating point, transient with trapezoidal integration, AC small-signal).

use crate::model::*;
use std::collections::HashMap;

const VT: f64 = 0.025852;
const GMIN: f64 = 1e-12;
pub const VOLTMETER_R: f64 = 10e6;
pub const DMM_VOLT_R: f64 = 1e9;
pub const OHM_TEST_I: f64 = 1e-3;

// ---------------------------------------------------------------- netlist

#[derive(Clone, Debug, Default)]
pub struct Netlist {
    /// Number of nets including ground (net 0).
    pub n_nets: usize,
    /// Net per component pin (same order as circuit.components).
    pub comp_nets: Vec<Vec<usize>>,
    /// How many pins touch each net.
    pub net_pin_count: Vec<usize>,
    /// Net index for each wire (usize::MAX when the wire touches no pin).
    pub wire_nets: Vec<usize>,
    pub has_ground: bool,
}

struct Dsu {
    p: Vec<usize>,
}
impl Dsu {
    fn new(n: usize) -> Self {
        Dsu { p: (0..n).collect() }
    }
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.p[r] != r {
            r = self.p[r];
        }
        let mut c = x;
        while self.p[c] != r {
            let n = self.p[c];
            self.p[c] = r;
            c = n;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.p[b] = a;
        }
    }
}

pub fn on_segment(p: GP, a: GP, b: GP) -> bool {
    if a.x == b.x && p.x == a.x {
        p.y >= a.y.min(b.y) && p.y <= a.y.max(b.y)
    } else if a.y == b.y && p.y == a.y {
        p.x >= a.x.min(b.x) && p.x <= a.x.max(b.x)
    } else {
        false
    }
}

pub fn build_netlist(c: &Circuit) -> Netlist {
    let mut ids: HashMap<GP, usize> = HashMap::new();
    let id_of = |p: GP, ids: &mut HashMap<GP, usize>| -> usize {
        let n = ids.len();
        *ids.entry(p).or_insert(n)
    };
    let mut pin_ids: Vec<Vec<usize>> = Vec::new();
    for comp in &c.components {
        pin_ids.push(comp.pin_positions().into_iter().map(|p| id_of(p, &mut ids)).collect());
    }
    let mut wire_ids: Vec<Vec<usize>> = Vec::new();
    for w in &c.wires {
        wire_ids.push(w.points.iter().map(|&p| id_of(p, &mut ids)).collect());
    }
    let mut dsu = Dsu::new(ids.len());
    for w in &wire_ids {
        for k in 1..w.len() {
            dsu.union(w[k - 1], w[k]);
        }
    }
    // T-junctions: wire endpoints or pins lying on another wire's segment
    let mut probes: Vec<GP> = Vec::new();
    for comp in &c.components {
        probes.extend(comp.pin_positions());
    }
    for w in &c.wires {
        if let (Some(f), Some(l)) = (w.points.first(), w.points.last()) {
            probes.push(*f);
            probes.push(*l);
        }
    }
    for p in probes {
        let pid = ids[&p];
        for (wi, w) in c.wires.iter().enumerate() {
            for k in 1..w.points.len() {
                if on_segment(p, w.points[k - 1], w.points[k]) {
                    dsu.union(pid, wire_ids[wi][k]);
                }
            }
        }
    }
    // ground roots
    let mut ground_roots = Vec::new();
    for (ci, comp) in c.components.iter().enumerate() {
        if comp.kind == Kind::Ground {
            ground_roots.push(dsu.find(pin_ids[ci][0]));
        }
    }
    let mut root_to_net: HashMap<usize, usize> = HashMap::new();
    for r in &ground_roots {
        root_to_net.insert(*r, 0);
    }
    let mut next = 1;
    let mut comp_nets = Vec::new();
    for pins in &pin_ids {
        let mut v = Vec::new();
        for &pid in pins {
            let r = dsu.find(pid);
            let n = *root_to_net.entry(r).or_insert_with(|| {
                next += 1;
                next - 1
            });
            v.push(n);
        }
        comp_nets.push(v);
    }
    let mut net_pin_count = vec![0; next];
    for v in &comp_nets {
        for &n in v {
            net_pin_count[n] += 1;
        }
    }
    let wire_nets = wire_ids
        .iter()
        .map(|w| {
            w.first()
                .and_then(|&id| root_to_net.get(&dsu.find(id)).copied())
                .unwrap_or(usize::MAX)
        })
        .collect();
    Netlist {
        n_nets: next,
        comp_nets,
        net_pin_count,
        wire_nets,
        has_ground: !ground_roots.is_empty(),
    }
}

// ---------------------------------------------------------------- elements

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Src {
    Dc(f64),
    Sine { amp: f64, freq: f64, phase: f64, offset: f64 },
    Fg { wave: Wave, amp: f64, freq: f64, offset: f64, duty: f64, invert: bool },
}

impl Src {
    pub fn at(&self, t: f64) -> f64 {
        match *self {
            Src::Dc(v) => v,
            Src::Sine { amp, freq, phase, offset } => {
                offset + amp * (2.0 * std::f64::consts::PI * freq * t + phase.to_radians()).sin()
            }
            Src::Fg { wave, amp, freq, offset, duty, invert } => {
                let ph = (t * freq).fract();
                let w = match wave {
                    Wave::Sine => (2.0 * std::f64::consts::PI * ph).sin(),
                    Wave::Triangle => {
                        // starts at 0 rising, like a sine
                        let p = (ph + 0.25).fract();
                        if p < 0.5 {
                            4.0 * p - 1.0
                        } else {
                            3.0 - 4.0 * p
                        }
                    }
                    Wave::Square => {
                        if ph < duty / 100.0 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                };
                let v = offset + amp * w;
                if invert {
                    v
                } else {
                    v
                }
            }
        }
    }
    pub fn ac_mag(&self) -> f64 {
        match *self {
            Src::Dc(_) => 0.0,
            Src::Sine { amp, .. } => amp,
            Src::Fg { amp, .. } => amp,
        }
    }
    pub fn ac_phase(&self) -> f64 {
        match *self {
            Src::Sine { phase, .. } => phase.to_radians(),
            _ => 0.0,
        }
    }
    pub fn freq(&self) -> f64 {
        match *self {
            Src::Dc(_) => 0.0,
            Src::Sine { freq, .. } => freq,
            Src::Fg { freq, .. } => freq,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EK {
    R(f64),
    C(f64),
    L(f64),
    V(Src),
    I(Src),
    D { is: f64, n: f64, bv: Option<f64> },
    /// Op-amp: output (node c) = vsat·tanh(gain·(V(a) − V(b)) / vsat).
    OpAmp { gain: f64, vsat: f64 },
}

#[derive(Clone, Debug)]
pub struct Elem {
    pub k: EK,
    pub a: usize,
    pub b: usize,
    pub branch: Option<usize>,
    pub comp: usize,
    pub role: u8,
    /// Third node (op-amp output).
    pub c: usize,
}

fn led_is(vf: f64) -> f64 {
    // forward voltage reached at 10 mA with n = 2
    0.01 / (vf / (2.0 * VT)).exp()
}

pub fn build_elems(c: &Circuit, nl: &Netlist) -> (Vec<Elem>, usize) {
    let mut els = Vec::new();
    let mut nb = 0usize;
    let mut br = || {
        nb += 1;
        Some(nb - 1)
    };
    for (ci, comp) in c.components.iter().enumerate() {
        let n = &nl.comp_nets[ci];
        let p = &comp.params;
        let mut opamp_out: Option<usize> = None;
        let ron = if p.r_internal > 0.0 { p.r_internal } else { 1e-3 };
        let mut push = |k: EK, a: usize, b: usize, branch: Option<usize>, role: u8| {
            els.push(Elem { k, a, b, branch, comp: ci, role, c: 0 });
        };
        match comp.kind {
            Kind::Resistor => push(EK::R(p.value.max(1e-6)), n[0], n[1], None, 0),
            Kind::Capacitor => push(EK::C(p.value.max(1e-18)), n[0], n[1], None, 0),
            Kind::Inductor => push(EK::L(p.value.max(1e-15)), n[0], n[1], br(), 0),
            Kind::Potentiometer => {
                let s = (p.setting / 100.0).clamp(0.0, 1.0);
                let r = p.value.max(1e-3);
                push(EK::R((r * s).max(1e-3)), n[0], n[2], None, 0);
                push(EK::R((r * (1.0 - s)).max(1e-3)), n[2], n[1], None, 1);
            }
            Kind::DcVoltage => push(EK::V(Src::Dc(p.value)), n[0], n[1], br(), 0),
            Kind::DcCurrent => push(EK::I(Src::Dc(p.value)), n[0], n[1], None, 0),
            Kind::AcVoltage => push(
                EK::V(Src::Sine { amp: p.value, freq: p.freq, phase: p.phase, offset: p.offset }),
                n[0],
                n[1],
                br(),
                0,
            ),
            Kind::Ground | Kind::Probe | Kind::Oscilloscope => {}
            Kind::Diode => push(EK::D { is: p.is, n: p.n, bv: if p.bv > 0.0 { Some(p.bv) } else { None } }, n[0], n[1], None, 0),
            Kind::Zener => push(EK::D { is: p.is, n: p.n, bv: Some(p.value.abs()) }, n[0], n[1], None, 0),
            Kind::Led => {
                let vf = if p.vf > 0.0 { p.vf } else { p.led.forward_voltage() };
                push(EK::D { is: led_is(vf), n: 2.0, bv: None }, n[0], n[1], None, 0)
            }
            Kind::Switch => push(EK::R(if p.closed { ron } else { 1e10 }), n[0], n[1], None, 0),
            Kind::PushButton => push(EK::R(if p.closed != p.normally_closed { ron } else { 1e10 }), n[0], n[1], None, 0),
            Kind::SpdtSwitch => {
                push(EK::R(if !p.closed { ron } else { 1e10 }), n[0], n[1], None, 0);
                push(EK::R(if p.closed { ron } else { 1e10 }), n[0], n[2], None, 1);
            }
            Kind::Fuse => push(EK::R(if p.blown { 1e10 } else { 1e-2 }), n[0], n[1], None, 0),
            Kind::Motor | Kind::Buzzer => push(EK::R(p.value.max(1e-3)), n[0], n[1], None, 0),
            Kind::ClockSource => push(
                EK::V(Src::Fg { wave: Wave::Square, amp: (p.value - p.offset) / 2.0, freq: p.freq, offset: (p.value + p.offset) / 2.0, duty: p.duty, invert: false }),
                n[0],
                n[1],
                br(),
                0,
            ),
            Kind::AcCurrent => push(EK::I(Src::Sine { amp: p.value, freq: p.freq, phase: p.phase, offset: p.offset }), n[0], n[1], None, 0),
            Kind::OpAmp => {
                push(EK::OpAmp { gain: p.value.max(1.0), vsat: p.power.abs().max(0.1) }, n[1], n[0], br(), 0);
                opamp_out = Some(n[2]);
            }
            Kind::Wattmeter => {
                push(EK::V(Src::Dc(0.0)), n[2], n[3], br(), 0);
                push(EK::R(DMM_VOLT_R), n[0], n[1], None, 1);
            }
            Kind::BodePlotter | Kind::FreqCounter => {}
            Kind::Lamp => {
                let r = if p.power > 0.0 { p.value * p.value / p.power } else { 1e6 };
                push(EK::R(r.max(1e-3)), n[0], n[1], None, 0)
            }
            Kind::Voltmeter => push(EK::R(if p.r_internal > 0.0 { p.r_internal } else { VOLTMETER_R }), n[0], n[1], None, 0),
            Kind::Ammeter => {
                if p.r_internal > 0.0 {
                    push(EK::R(p.r_internal), n[0], n[1], None, 0)
                } else {
                    push(EK::V(Src::Dc(0.0)), n[0], n[1], br(), 0)
                }
            }
            Kind::Multimeter => match p.meter {
                MeterMode::Volts | MeterMode::Decibel => push(EK::R(if p.r_internal > 0.0 { p.r_internal } else { DMM_VOLT_R }), n[0], n[1], None, 0),
                MeterMode::Amps => {
                    if p.r_amm > 0.0 {
                        push(EK::R(p.r_amm), n[0], n[1], None, 0)
                    } else {
                        push(EK::V(Src::Dc(0.0)), n[0], n[1], br(), 0)
                    }
                }
                MeterMode::Ohms => push(EK::I(Src::Dc(if p.i_test > 0.0 { p.i_test } else { OHM_TEST_I })), n[0], n[1], None, 0),
            },
            Kind::FuncGen => {
                let s = Src::Fg { wave: p.wave, amp: p.value, freq: p.freq, offset: p.offset, duty: p.duty, invert: false };
                push(EK::V(s), n[0], n[1], br(), 0);
                push(EK::V(s), n[1], n[2], br(), 1);
            }
        }
        if let Some(o) = opamp_out {
            if let Some(last) = els.last_mut() {
                last.c = o;
            }
        }
    }
    (els, nb)
}

// ---------------------------------------------------------------- linear algebra

/// Solve A x = b in place (A is n*n row-major). Returns false if singular.
pub fn lu_solve(a: &mut [f64], b: &mut [f64], n: usize) -> bool {
    for k in 0..n {
        let mut piv = k;
        let mut best = a[k * n + k].abs();
        for r in k + 1..n {
            let v = a[r * n + k].abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if best < 1e-300 {
            return false;
        }
        if piv != k {
            for c in 0..n {
                a.swap(k * n + c, piv * n + c);
            }
            b.swap(k, piv);
        }
        let d = a[k * n + k];
        for r in k + 1..n {
            let f = a[r * n + k] / d;
            if f != 0.0 {
                for c in k..n {
                    a[r * n + c] -= f * a[k * n + c];
                }
                b[r] -= f * b[k];
            }
        }
    }
    for k in (0..n).rev() {
        let mut s = b[k];
        for c in k + 1..n {
            s -= a[k * n + c] * b[c];
        }
        b[k] = s / a[k * n + k];
    }
    b.iter().all(|v| v.is_finite())
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cx {
    pub re: f64,
    pub im: f64,
}
impl Cx {
    pub fn new(re: f64, im: f64) -> Self {
        Cx { re, im }
    }
    fn add(self, o: Cx) -> Cx {
        Cx::new(self.re + o.re, self.im + o.im)
    }
    fn sub(self, o: Cx) -> Cx {
        Cx::new(self.re - o.re, self.im - o.im)
    }
    fn mul(self, o: Cx) -> Cx {
        Cx::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
    fn div(self, o: Cx) -> Cx {
        let d = o.re * o.re + o.im * o.im;
        Cx::new((self.re * o.re + self.im * o.im) / d, (self.im * o.re - self.re * o.im) / d)
    }
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
}

fn cx_solve(a: &mut [Cx], b: &mut [Cx], n: usize) -> bool {
    for k in 0..n {
        let mut piv = k;
        let mut best = a[k * n + k].abs();
        for r in k + 1..n {
            let v = a[r * n + k].abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if best < 1e-300 {
            return false;
        }
        if piv != k {
            for c in 0..n {
                a.swap(k * n + c, piv * n + c);
            }
            b.swap(k, piv);
        }
        let d = a[k * n + k];
        for r in k + 1..n {
            let f = a[r * n + k].div(d);
            if f.abs() != 0.0 {
                for c in k..n {
                    a[r * n + c] = a[r * n + c].sub(f.mul(a[k * n + c]));
                }
                b[r] = b[r].sub(f.mul(b[k]));
            }
        }
    }
    for k in (0..n).rev() {
        let mut s = b[k];
        for c in k + 1..n {
            s = s.sub(a[k * n + c].mul(b[c]));
        }
        b[k] = s.div(a[k * n + k]);
    }
    true
}

// ---------------------------------------------------------------- diode helpers

fn diode_eval(vd: f64, is: f64, n: f64, bv: Option<f64>) -> (f64, f64) {
    let vt = n * VT;
    let (mut id, mut gd);
    if vd >= -5.0 * vt {
        let e = (vd / vt).min(90.0).exp();
        id = is * (e - 1.0);
        gd = is * e / vt;
    } else {
        id = -is;
        gd = is / vt * (-5.0f64).exp();
    }
    if let Some(bv) = bv {
        let ibv = 1e-3;
        let e = ((-(vd + bv)) / VT).min(90.0).exp();
        id -= ibv * e;
        gd += ibv * e / VT;
    }
    (id + GMIN * vd, gd + GMIN)
}

fn pnjlim(vnew: f64, vold: f64, vt: f64, vcrit: f64) -> f64 {
    if vnew > vcrit && (vnew - vold).abs() > 2.0 * vt {
        if vold > 0.0 {
            let arg = 1.0 + (vnew - vold) / vt;
            if arg > 0.0 {
                vold + vt * arg.ln()
            } else {
                vcrit
            }
        } else {
            vt * (vnew / vt).max(1e-30).ln()
        }
    } else {
        vnew
    }
}

fn limit_diode(vnew: f64, vold: f64, is: f64, n: f64, bv: Option<f64>) -> f64 {
    let vt = n * VT;
    let vcrit = vt * (vt / (std::f64::consts::SQRT_2 * is)).ln();
    let mut v = pnjlim(vnew, vold, vt, vcrit);
    if let Some(bv) = bv {
        let ibv: f64 = 1e-3;
        let vcr = VT * (VT / (std::f64::consts::SQRT_2 * ibv)).ln();
        let rn = -(v + bv);
        let ro = -(vold + bv);
        let rl = pnjlim(rn, ro, VT, vcr.max(0.0));
        v = -rl - bv;
    }
    v
}

// ---------------------------------------------------------------- engine

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Dc,
    Tran { h: f64, be: bool },
}

#[derive(Clone, Debug, Default)]
struct EState {
    v_prev: f64,
    i_prev: f64,
    vd: f64,
}

#[derive(Clone)]
pub struct Sim {
    pub nl: Netlist,
    pub els: Vec<Elem>,
    pub nb: usize,
    pub nn: usize,
    pub x: Vec<f64>,
    pub t: f64,
    st: Vec<EState>,
    pub be_next: bool,
    pub nonlinear: bool,
    pub src_scale: f64,
}

impl Sim {
    pub fn new(c: &Circuit) -> Result<Sim, String> {
        Self::with_netlist(c, build_netlist(c))
    }

    /// Build a simulator from a ready-made netlist (used by the breadboard).
    pub fn with_netlist(c: &Circuit, nl: Netlist) -> Result<Sim, String> {
        if !nl.has_ground {
            return Err("The circuit has no ground. Place a Ground part and wire it to the circuit.".into());
        }
        let has_parts = c.components.iter().any(|p| !matches!(p.kind, Kind::Ground | Kind::Probe | Kind::Oscilloscope));
        if !has_parts {
            return Err("There is nothing to simulate yet. Place some parts first.".into());
        }
        let (els, nb) = build_elems(c, &nl);
        let nn = nl.n_nets.saturating_sub(1);
        let nonlinear = els.iter().any(|e| matches!(e.k, EK::D { .. } | EK::OpAmp { .. }));
        let mut st = vec![EState::default(); els.len()];
        for (i, e) in els.iter().enumerate() {
            let comp = &c.components[e.comp];
            match e.k {
                EK::C(_) => st[i].v_prev = comp.params.ic,
                EK::L(_) => st[i].i_prev = comp.params.ic,
                _ => {}
            }
        }
        Ok(Sim { nl, x: vec![0.0; nn + nb], els, nb, nn, t: 0.0, st, be_next: true, nonlinear, src_scale: 1.0 })
    }

    /// Re-read part values (switch states, generator settings...) while running.
    /// Returns false when the structure changed and the sim should restart.
    pub fn refresh(&mut self, c: &Circuit) -> bool {
        self.refresh_with(c, build_netlist(c))
    }

    pub fn refresh_with(&mut self, c: &Circuit, nl: Netlist) -> bool {
        if nl.comp_nets != self.nl.comp_nets {
            return false;
        }
        let (els, nb) = build_elems(c, &nl);
        if nb != self.nb || els.len() != self.els.len() {
            return false;
        }
        for (old, new) in self.els.iter().zip(els.iter()) {
            if std::mem::discriminant(&old.k) != std::mem::discriminant(&new.k) {
                return false;
            }
        }
        // a switch flipping or a resistance jump is a discontinuity: restart integration with BE
        for (old, new) in self.els.iter().zip(els.iter()) {
            if let (EK::R(a), EK::R(b)) = (&old.k, &new.k) {
                if (a - b).abs() > 1e-12 * a.abs().max(1.0) {
                    self.be_next = true;
                }
            }
        }
        self.els = els;
        true
    }

    fn n(&self) -> usize {
        self.nn + self.nb
    }

    pub fn v(&self, net: usize) -> f64 {
        if net == 0 || net > self.nn {
            0.0
        } else {
            self.x[net - 1]
        }
    }

    fn vx(x: &[f64], net: usize) -> f64 {
        if net == 0 {
            0.0
        } else {
            x[net - 1]
        }
    }

    /// Current through element i, flowing from pin a to pin b.
    pub fn elem_current(&self, i: usize) -> f64 {
        let e = &self.els[i];
        let vab = self.v(e.a) - self.v(e.b);
        match e.k {
            EK::R(r) => vab / r,
            EK::C(_) => self.st[i].i_prev,
            EK::L(_) | EK::V(_) => e.branch.map(|k| self.x[self.nn + k]).unwrap_or(0.0),
            EK::I(s) => -s.at(self.t) * self.src_scale,
            EK::D { is, n, bv } => diode_eval(vab, is, n, bv).0,
            EK::OpAmp { .. } => e.branch.map(|k| self.x[self.nn + k]).unwrap_or(0.0),
        }
    }

    /// Main current through a component (its first element), from pin 1 to pin 2.
    pub fn comp_current(&self, comp: usize) -> f64 {
        for (i, e) in self.els.iter().enumerate() {
            if e.comp == comp && e.role == 0 {
                return self.elem_current(i);
            }
        }
        0.0
    }

    fn stamp_g(a_m: &mut [f64], n: usize, nn: usize, a: usize, b: usize, g: f64) {
        let _ = nn;
        if a > 0 {
            a_m[(a - 1) * n + (a - 1)] += g;
        }
        if b > 0 {
            a_m[(b - 1) * n + (b - 1)] += g;
        }
        if a > 0 && b > 0 {
            a_m[(a - 1) * n + (b - 1)] -= g;
            a_m[(b - 1) * n + (a - 1)] -= g;
        }
    }

    /// Current `i` injected into node a and drawn from node b.
    fn stamp_i(rhs: &mut [f64], a: usize, b: usize, i: f64) {
        if a > 0 {
            rhs[a - 1] += i;
        }
        if b > 0 {
            rhs[b - 1] -= i;
        }
    }

    fn stamp_branch(a_m: &mut [f64], n: usize, a: usize, b: usize, k: usize) {
        if a > 0 {
            a_m[(a - 1) * n + k] += 1.0;
            a_m[k * n + (a - 1)] += 1.0;
        }
        if b > 0 {
            a_m[(b - 1) * n + k] -= 1.0;
            a_m[k * n + (b - 1)] -= 1.0;
        }
    }

    /// Solve one operating point (DC or a transient time point) with Newton iterations.
    fn solve(&mut self, mode: Mode, t: f64) -> Result<Vec<f64>, String> {
        let n = self.n();
        if n == 0 {
            return Ok(vec![]);
        }
        let nn = self.nn;
        let mut x = self.x.clone();
        let mut vd: Vec<f64> = self.st.iter().map(|s| s.vd).collect();
        let max_iter = if self.nonlinear { 200 } else { 1 };
        let mut am = vec![0.0; n * n];
        let mut rhs = vec![0.0; n];
        for iter in 0..max_iter {
            am.iter_mut().for_each(|v| *v = 0.0);
            rhs.iter_mut().for_each(|v| *v = 0.0);
            for i in 0..nn {
                am[i * n + i] += GMIN;
            }
            for (ei, e) in self.els.iter().enumerate() {
                match e.k {
                    EK::R(r) => Self::stamp_g(&mut am, n, nn, e.a, e.b, 1.0 / r),
                    EK::C(c) => {
                        if let Mode::Tran { h, be } = mode {
                            let s = &self.st[ei];
                            let (g, ieq) = if be {
                                let g = c / h;
                                (g, g * s.v_prev)
                            } else {
                                let g = 2.0 * c / h;
                                (g, g * s.v_prev + s.i_prev)
                            };
                            Self::stamp_g(&mut am, n, nn, e.a, e.b, g);
                            Self::stamp_i(&mut rhs, e.a, e.b, ieq);
                        }
                    }
                    EK::L(l) => {
                        let k = nn + e.branch.unwrap();
                        Self::stamp_branch(&mut am, n, e.a, e.b, k);
                        if let Mode::Tran { h, be } = mode {
                            let s = &self.st[ei];
                            if be {
                                let req = l / h;
                                am[k * n + k] -= req;
                                rhs[k] = -req * s.i_prev;
                            } else {
                                let req = 2.0 * l / h;
                                am[k * n + k] -= req;
                                rhs[k] = -req * s.i_prev - s.v_prev;
                            }
                        }
                    }
                    EK::V(s) => {
                        let k = nn + e.branch.unwrap();
                        Self::stamp_branch(&mut am, n, e.a, e.b, k);
                        rhs[k] = s.at(t) * self.src_scale;
                    }
                    EK::I(s) => {
                        Self::stamp_i(&mut rhs, e.a, e.b, s.at(t) * self.src_scale);
                    }
                    EK::D { is, n: nd, bv } => {
                        let raw = Self::vx(&x, e.a) - Self::vx(&x, e.b);
                        let v = if iter == 0 { vd[ei] } else { limit_diode(raw, vd[ei], is, nd, bv) };
                        vd[ei] = v;
                        let (id, gd) = diode_eval(v, is, nd, bv);
                        Self::stamp_g(&mut am, n, nn, e.a, e.b, gd);
                        Self::stamp_i(&mut rhs, e.a, e.b, -(id - gd * v));
                    }
                    EK::OpAmp { gain, vsat } => {
                        let k = nn + e.branch.unwrap();
                        if e.c > 0 {
                            am[(e.c - 1) * n + k] += 1.0;
                            am[k * n + (e.c - 1)] += 1.0;
                        }
                        let raw = Self::vx(&x, e.a) - Self::vx(&x, e.b);
                        let v = if iter == 0 {
                            vd[ei]
                        } else {
                            // damp steps through the high-gain linear region
                            let lim = 2.0 * vsat / gain;
                            let x_old = gain * vd[ei] / vsat;
                            if x_old.abs() < 4.0 || (gain * raw / vsat).abs() < 4.0 {
                                vd[ei] + (raw - vd[ei]).clamp(-lim, lim)
                            } else {
                                raw
                            }
                        };
                        vd[ei] = v;
                        let th = (gain * v / vsat).tanh();
                        let g = gain * (1.0 - th * th);
                        if e.a > 0 {
                            am[k * n + (e.a - 1)] -= g;
                        }
                        if e.b > 0 {
                            am[k * n + (e.b - 1)] += g;
                        }
                        rhs[k] = vsat * th - g * v;
                    }
                }
            }
            let mut sol = rhs.clone();
            let mut a2 = am.clone();
            if !lu_solve(&mut a2, &mut sol, n) {
                return Err("The circuit equations could not be solved (a loop of voltage sources or a part with no path to ground?).".into());
            }
            let mut converged = true;
            for i in 0..n {
                let d = (sol[i] - x[i]).abs();
                if d > 1e-9 + 1e-6 * sol[i].abs() {
                    converged = false;
                    break;
                }
            }
            if self.nonlinear && converged {
                // also check diode voltages settled
                for (ei, e) in self.els.iter().enumerate() {
                    let raw = Self::vx(&sol, e.a) - Self::vx(&sol, e.b);
                    match e.k {
                        EK::D { .. } => {
                            if (raw - vd[ei]).abs() > 1e-6 + 1e-6 * raw.abs() {
                                converged = false;
                            }
                        }
                        EK::OpAmp { gain, vsat } => {
                            if vsat * ((gain * raw / vsat).tanh() - (gain * vd[ei] / vsat).tanh()).abs() > 1e-6 {
                                converged = false;
                            }
                        }
                        _ => {}
                    }
                }
            }
            x = sol;
            if !self.nonlinear || (converged && iter > 0) {
                for (ei, e) in self.els.iter().enumerate() {
                    if matches!(e.k, EK::D { .. } | EK::OpAmp { .. }) {
                        self.st[ei].vd = Self::vx(&x, e.a) - Self::vx(&x, e.b);
                    }
                }
                return Ok(x);
            }
        }
        Err("The simulation did not converge at this time point.".into())
    }

    /// DC operating point (capacitors open, inductors shorted). Updates the state.
    pub fn dc_op(&mut self) -> Result<(), String> {
        let saved = self.x.clone();
        match self.solve(Mode::Dc, 0.0) {
            Ok(x) => {
                self.x = x;
            }
            Err(_) => {
                // source stepping
                self.x = vec![0.0; self.n()];
                for s in self.st.iter_mut() {
                    s.vd = 0.0;
                }
                let mut ok = true;
                for k in 1..=20 {
                    self.src_scale = k as f64 / 20.0;
                    match self.solve(Mode::Dc, 0.0) {
                        Ok(x) => self.x = x,
                        Err(_) => {
                            ok = false;
                            break;
                        }
                    }
                }
                self.src_scale = 1.0;
                if !ok {
                    self.x = saved;
                    return Err("DC operating point did not converge.".into());
                }
            }
        }
        // initialise reactive state from the operating point
        for (i, e) in self.els.iter().enumerate() {
            let vab = Self::vx(&self.x, e.a) - Self::vx(&self.x, e.b);
            match e.k {
                EK::C(_) => {
                    self.st[i].v_prev = vab;
                    self.st[i].i_prev = 0.0;
                }
                EK::L(_) => {
                    self.st[i].i_prev = self.x[self.nn + e.branch.unwrap()];
                    self.st[i].v_prev = 0.0;
                }
                _ => {}
            }
        }
        self.be_next = true;
        Ok(())
    }

    /// Advance the transient simulation by one time step h.
    pub fn step(&mut self, h: f64) -> Result<(), String> {
        let be = self.be_next;
        let t1 = self.t + h;
        let x = match self.solve(Mode::Tran { h, be }, t1) {
            Ok(x) => x,
            Err(e) => {
                // retry with smaller steps
                let mut ok = false;
                let sub = h / 10.0;
                for _ in 0..10 {
                    match self.solve(Mode::Tran { h: sub, be: true }, self.t + sub) {
                        Ok(x) => {
                            self.accept(x, sub, true);
                            ok = true;
                        }
                        Err(_) => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    return Err(e);
                }
                return Ok(());
            }
        };
        self.accept(x, h, be);
        Ok(())
    }

    fn accept(&mut self, x: Vec<f64>, h: f64, be: bool) {
        for (i, e) in self.els.iter().enumerate() {
            let vab = Self::vx(&x, e.a) - Self::vx(&x, e.b);
            match e.k {
                EK::C(c) => {
                    let s = &mut self.st[i];
                    let i_new = if be {
                        c / h * (vab - s.v_prev)
                    } else {
                        2.0 * c / h * (vab - s.v_prev) - s.i_prev
                    };
                    s.v_prev = vab;
                    s.i_prev = i_new;
                }
                EK::L(_) => {
                    let s = &mut self.st[i];
                    s.i_prev = x[self.nn + e.branch.unwrap()];
                    s.v_prev = vab;
                }
                _ => {}
            }
        }
        self.x = x;
        self.t += h;
        self.be_next = false;
    }

    /// Highest source frequency in the circuit (0 when DC only).
    pub fn max_freq(&self) -> f64 {
        self.els
            .iter()
            .map(|e| match &e.k {
                EK::V(s) | EK::I(s) => s.freq(),
                _ => 0.0,
            })
            .fold(0.0, f64::max)
    }

    /// Small-signal AC solution at frequency f (requires a DC operating point first).
    pub fn ac_point(&self, f: f64) -> Option<Vec<Cx>> {
        let n = self.n();
        let nn = self.nn;
        let w = 2.0 * std::f64::consts::PI * f;
        let mut a = vec![Cx::default(); n * n];
        let mut b = vec![Cx::default(); n];
        let add = |a: &mut Vec<Cx>, r: usize, c: usize, v: Cx| {
            a[r * n + c] = a[r * n + c].add(v);
        };
        let stamp_y = |a: &mut Vec<Cx>, p: usize, q: usize, y: Cx| {
            if p > 0 {
                add(a, p - 1, p - 1, y);
            }
            if q > 0 {
                add(a, q - 1, q - 1, y);
            }
            if p > 0 && q > 0 {
                add(a, p - 1, q - 1, Cx::new(-y.re, -y.im));
                add(a, q - 1, p - 1, Cx::new(-y.re, -y.im));
            }
        };
        let stamp_br = |a: &mut Vec<Cx>, p: usize, q: usize, k: usize| {
            if p > 0 {
                add(a, p - 1, k, Cx::new(1.0, 0.0));
                add(a, k, p - 1, Cx::new(1.0, 0.0));
            }
            if q > 0 {
                add(a, q - 1, k, Cx::new(-1.0, 0.0));
                add(a, k, q - 1, Cx::new(-1.0, 0.0));
            }
        };
        for i in 0..nn {
            add(&mut a, i, i, Cx::new(GMIN, 0.0));
        }
        for (ei, e) in self.els.iter().enumerate() {
            match e.k {
                EK::R(r) => stamp_y(&mut a, e.a, e.b, Cx::new(1.0 / r, 0.0)),
                EK::C(c) => stamp_y(&mut a, e.a, e.b, Cx::new(0.0, w * c)),
                EK::L(l) => {
                    let k = nn + e.branch.unwrap();
                    stamp_br(&mut a, e.a, e.b, k);
                    add(&mut a, k, k, Cx::new(0.0, -w * l));
                }
                EK::V(s) => {
                    let k = nn + e.branch.unwrap();
                    stamp_br(&mut a, e.a, e.b, k);
                    let m = s.ac_mag();
                    let ph = s.ac_phase();
                    // the inverted output of a function generator is in phase with "+" by construction
                    b[k] = Cx::new(m * ph.cos(), m * ph.sin());
                }
                EK::I(s) => {
                    let m = s.ac_mag();
                    if m != 0.0 {
                        if e.a > 0 {
                            b[e.a - 1] = b[e.a - 1].add(Cx::new(m, 0.0));
                        }
                        if e.b > 0 {
                            b[e.b - 1] = b[e.b - 1].sub(Cx::new(m, 0.0));
                        }
                    }
                }
                EK::D { is, n: nd, bv } => {
                    let (_, gd) = diode_eval(self.st[ei].vd, is, nd, bv);
                    stamp_y(&mut a, e.a, e.b, Cx::new(gd, 0.0));
                }
                EK::OpAmp { gain, vsat } => {
                    let k = nn + e.branch.unwrap();
                    if e.c > 0 {
                        add(&mut a, e.c - 1, k, Cx::new(1.0, 0.0));
                        add(&mut a, k, e.c - 1, Cx::new(1.0, 0.0));
                    }
                    let th = (gain * self.st[ei].vd / vsat).tanh();
                    let g = gain * (1.0 - th * th);
                    if e.a > 0 {
                        add(&mut a, k, e.a - 1, Cx::new(-g, 0.0));
                    }
                    if e.b > 0 {
                        add(&mut a, k, e.b - 1, Cx::new(g, 0.0));
                    }
                }
            }
        }
        if cx_solve(&mut a, &mut b, n) {
            Some(b)
        } else {
            None
        }
    }

    pub fn set_dc_source(&mut self, comp: usize, value: f64) {
        for e in self.els.iter_mut() {
            if e.comp == comp {
                match &mut e.k {
                    EK::V(s) | EK::I(s) => *s = Src::Dc(value),
                    _ => {}
                }
            }
        }
    }
}

// ---------------------------------------------------------------- analyses

/// A quantity that can be plotted.
#[derive(Clone, Debug, PartialEq)]
pub enum Var {
    Node(usize),
    Current { comp: usize },
}

pub fn var_name(c: &Circuit, v: &Var) -> String {
    match v {
        Var::Node(n) => format!("V({})", n),
        Var::Current { comp } => format!("I({})", c.components.get(*comp).map(|x| x.refdes.as_str()).unwrap_or("?")),
    }
}

/// All plottable variables: every non-ground net, and currents through sources and passive parts.
pub fn all_vars(c: &Circuit, nl: &Netlist) -> Vec<Var> {
    let mut v: Vec<Var> = (1..nl.n_nets).map(Var::Node).collect();
    for (i, comp) in c.components.iter().enumerate() {
        match comp.kind {
            Kind::Resistor
            | Kind::Capacitor
            | Kind::Inductor
            | Kind::DcVoltage
            | Kind::AcVoltage
            | Kind::Diode
            | Kind::Zener
            | Kind::Led
            | Kind::Lamp
            | Kind::Ammeter
            | Kind::Switch
            | Kind::PushButton
            | Kind::Fuse
            | Kind::Motor
            | Kind::Buzzer
            | Kind::OpAmp
            | Kind::AcCurrent
            | Kind::ClockSource => v.push(Var::Current { comp: i }),
            _ => {}
        }
    }
    v
}

pub fn eval_var(sim: &Sim, v: &Var) -> f64 {
    match v {
        Var::Node(n) => sim.v(*n),
        Var::Current { comp } => sim.comp_current(*comp),
    }
}

pub struct OpResult {
    pub rows: Vec<(String, f64, &'static str)>,
}

pub fn run_dc_op(c: &Circuit) -> Result<OpResult, String> {
    let mut sim = Sim::new(c)?;
    sim.dc_op()?;
    let mut rows = Vec::new();
    for v in all_vars(c, &sim.nl) {
        let unit = if matches!(v, Var::Node(_)) { "V" } else { "A" };
        rows.push((var_name(c, &v), eval_var(&sim, &v), unit));
    }
    Ok(OpResult { rows })
}

pub struct Series {
    pub name: String,
    pub pts: Vec<[f64; 2]>,
}

pub fn run_transient(c: &Circuit, tstop: f64, tstep: f64, use_op: bool, vars: &[Var]) -> Result<Vec<Series>, String> {
    let mut sim = Sim::new(c)?;
    if use_op {
        sim.dc_op()?;
    }
    let mut out: Vec<Series> = vars.iter().map(|v| Series { name: var_name(c, v), pts: Vec::new() }).collect();
    let steps = ((tstop / tstep).ceil() as usize).clamp(1, 2_000_000);
    let keep_every = (steps / 20_000).max(1);
    for (s, v) in out.iter_mut().zip(vars) {
        s.pts.push([0.0, eval_var(&sim, v)]);
    }
    for k in 1..=steps {
        sim.step(tstep)?;
        if k % keep_every == 0 || k == steps {
            for (s, v) in out.iter_mut().zip(vars) {
                s.pts.push([sim.t, eval_var(&sim, v)]);
            }
        }
    }
    Ok(out)
}

pub struct AcSeries {
    pub name: String,
    pub mag: Vec<[f64; 2]>,
    pub phase: Vec<[f64; 2]>,
}

pub fn run_ac(c: &Circuit, fstart: f64, fstop: f64, ppd: usize, vars: &[Var]) -> Result<Vec<AcSeries>, String> {
    let mut sim = Sim::new(c)?;
    sim.dc_op()?;
    let has_ac = sim.els.iter().any(|e| match &e.k {
        EK::V(s) | EK::I(s) => s.ac_mag() != 0.0,
        _ => false,
    });
    if !has_ac {
        return Err("AC sweep needs an AC source (AC voltage source or function generator).".into());
    }
    let mut out: Vec<AcSeries> = vars
        .iter()
        .map(|v| AcSeries { name: var_name(c, v), mag: vec![], phase: vec![] })
        .collect();
    let (l0, l1) = (fstart.max(1e-3).log10(), fstop.max(fstart * 1.0001).log10());
    let npts = (((l1 - l0) * ppd as f64).ceil() as usize).max(2);
    for i in 0..=npts {
        let lf = l0 + (l1 - l0) * i as f64 / npts as f64;
        let f = 10f64.powf(lf);
        let sol = sim.ac_point(f).ok_or("AC equations could not be solved.")?;
        for (s, v) in out.iter_mut().zip(vars) {
            let z = match v {
                Var::Node(n) => {
                    if *n == 0 {
                        Cx::default()
                    } else {
                        sol[n - 1]
                    }
                }
                Var::Current { comp } => {
                    // current through first element of the component
                    let mut z = Cx::default();
                    for e in sim.els.iter() {
                        if e.comp == *comp && e.role == 0 {
                            let va = if e.a == 0 { Cx::default() } else { sol[e.a - 1] };
                            let vb = if e.b == 0 { Cx::default() } else { sol[e.b - 1] };
                            let vab = va.sub(vb);
                            let w = 2.0 * std::f64::consts::PI * f;
                            z = match e.k {
                                EK::R(r) => Cx::new(vab.re / r, vab.im / r),
                                EK::C(cap) => vab.mul(Cx::new(0.0, w * cap)),
                                EK::L(_) | EK::V(_) => sol[sim.nn + e.branch.unwrap()],
                                _ => Cx::default(),
                            };
                            break;
                        }
                    }
                    z
                }
            };
            let m = z.abs();
            s.mag.push([lf, 20.0 * m.max(1e-30).log10()]);
            s.phase.push([lf, z.arg().to_degrees()]);
        }
    }
    Ok(out)
}

/// Frequency response V(OUT)/V(IN) seen by a Bode plotter: (dB points, phase points) over log10(f).
pub fn run_bode(c: &Circuit, comp: usize, fstart: f64, fstop: f64, ppd: usize) -> Result<(Vec<[f64; 2]>, Vec<[f64; 2]>), String> {
    let mut sim = Sim::new(c)?;
    sim.dc_op()?;
    let has_ac = sim.els.iter().any(|e| match &e.k {
        EK::V(s) | EK::I(s) => s.ac_mag() != 0.0,
        _ => false,
    });
    if !has_ac {
        return Err("Add an AC source or function generator to drive the circuit.".into());
    }
    let nets = sim.nl.comp_nets[comp].clone();
    let node = |sol: &Vec<Cx>, n: usize| if n == 0 { Cx::default() } else { sol[n - 1] };
    let (l0, l1) = (fstart.max(1e-3).log10(), fstop.max(fstart.max(1e-3) * 1.01).log10());
    let npts = (((l1 - l0) * ppd.max(2) as f64).ceil() as usize).clamp(2, 4000);
    let mut mag = Vec::new();
    let mut ph = Vec::new();
    let mut last_phase: Option<f64> = None;
    for i in 0..=npts {
        let lf = l0 + (l1 - l0) * i as f64 / npts as f64;
        let sol = sim.ac_point(10f64.powf(lf)).ok_or("AC equations could not be solved.")?;
        let vin = node(&sol, nets[0]);
        let vout = node(&sol, nets[1]);
        if vin.abs() < 1e-15 {
            return Err("The IN pin sees no AC signal. Connect IN to the signal source.".into());
        }
        let h = vout.div(vin);
        mag.push([lf, 20.0 * h.abs().max(1e-30).log10()]);
        let mut p = h.arg().to_degrees();
        if let Some(lp) = last_phase {
            while p - lp > 180.0 {
                p -= 360.0;
            }
            while p - lp < -180.0 {
                p += 360.0;
            }
        }
        last_phase = Some(p);
        ph.push([lf, p]);
    }
    Ok((mag, ph))
}

pub fn run_dc_sweep(c: &Circuit, comp: usize, start: f64, stop: f64, incr: f64, vars: &[Var]) -> Result<Vec<Series>, String> {
    let mut sim = Sim::new(c)?;
    match c.components.get(comp).map(|k| k.kind) {
        Some(Kind::DcVoltage) | Some(Kind::DcCurrent) => {}
        _ => return Err("Choose a DC voltage or DC current source to sweep.".into()),
    }
    if incr <= 0.0 {
        return Err("The increment must be greater than zero.".into());
    }
    let mut out: Vec<Series> = vars.iter().map(|v| Series { name: var_name(c, v), pts: Vec::new() }).collect();
    let n = (((stop - start) / incr).abs().floor() as usize).min(100_000);
    let dir = if stop >= start { 1.0 } else { -1.0 };
    for i in 0..=n {
        let val = start + dir * incr * i as f64;
        sim.set_dc_source(comp, val);
        sim.dc_op()?;
        for (s, v) in out.iter_mut().zip(vars) {
            s.pts.push([val, eval_var(&sim, v)]);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn divider() -> Circuit {
        let mut c = Circuit::default();
        // V1 vertical at (0,0): + at (0,-3), - at (0,3)
        c.add(Kind::DcVoltage, GP::new(0, 0), 0);
        c.components[0].params.value = 10.0;
        // R1 horizontal at (5,-3): pins (2,-3) (8,-3)
        c.add(Kind::Resistor, GP::new(5, -3), 0);
        // R2 vertical at (8,0): rot 1 -> pins (8,-3) (8,3)
        c.add(Kind::Resistor, GP::new(8, 0), 1);
        c.add(Kind::Ground, GP::new(0, 3), 0);
        c.add_wire(vec![GP::new(0, -3), GP::new(2, -3)]);
        c.add_wire(vec![GP::new(0, 3), GP::new(8, 3)]);
        c
    }

    #[test]
    fn divider_op() {
        let c = divider();
        let mut s = Sim::new(&c).unwrap();
        s.dc_op().unwrap();
        let n_mid = s.nl.comp_nets[1][1];
        assert!((s.v(n_mid) - 5.0).abs() < 1e-6, "{}", s.v(n_mid));
    }

    #[test]
    fn diode_forward() {
        let mut c = divider();
        c.components[2].kind = Kind::Diode;
        c.components[2].params = Kind::Diode.default_params();
        let mut s = Sim::new(&c).unwrap();
        s.dc_op().unwrap();
        let n_mid = s.nl.comp_nets[1][1];
        let v = s.v(n_mid);
        assert!(v > 0.5 && v < 0.9, "{}", v);
    }

    #[test]
    fn rc_charge() {
        let mut c = divider();
        c.components[2].kind = Kind::Capacitor;
        c.components[2].params = Kind::Capacitor.default_params(); // 1uF, R=1k => tau 1ms
        let mut s = Sim::new(&c).unwrap();
        for _ in 0..1000 {
            s.step(1e-6).unwrap();
        }
        let n_mid = s.nl.comp_nets[1][1];
        let v = s.v(n_mid);
        assert!((v - 10.0 * (1.0 - (-1.0f64).exp())).abs() < 0.02, "{}", v);
    }
}
