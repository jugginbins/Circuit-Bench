//! Ready-made example circuits.

use crate::model::*;

fn pin(c: &Circuit, id: u64, k: usize) -> GP {
    c.comp(id).unwrap().pin_positions()[k]
}

/// Wire from a pin through optional corner points to another pin.
fn wire(c: &mut Circuit, from: (u64, usize), via: &[(i32, i32)], to: (u64, usize)) {
    let mut pts = vec![pin(c, from.0, from.1)];
    pts.extend(via.iter().map(|&(x, y)| GP::new(x, y)));
    pts.push(pin(c, to.0, to.1));
    c.add_wire(ortho(pts));
}

/// Insert elbows so every segment is horizontal or vertical.
pub fn ortho(pts: Vec<GP>) -> Vec<GP> {
    let mut out: Vec<GP> = Vec::new();
    for p in pts {
        if let Some(&a) = out.last() {
            if a.x != p.x && a.y != p.y {
                out.push(GP::new(p.x, a.y));
            }
        }
        out.push(p);
    }
    out
}

fn wire_pts(c: &mut Circuit, pts: &[(i32, i32)]) {
    c.add_wire(pts.iter().map(|&(x, y)| GP::new(x, y)).collect());
}

pub const NAMES: [&str; 9] = [
    "RC low-pass filter",
    "Voltage divider",
    "Half-wave rectifier",
    "LED with switch",
    "Series RLC resonance",
    "Zener regulator",
    "Lamp dimmer",
    "Op-amp amplifier",
    "Motor, buzzer and fuse",
];

pub fn by_index(i: usize) -> Circuit {
    match i {
        0 => rc_filter(),
        1 => divider(),
        2 => rectifier(),
        3 => led_switch(),
        4 => rlc(),
        5 => zener(),
        6 => dimmer(),
        7 => opamp(),
        _ => motor(),
    }
}

pub fn rc_filter() -> Circuit {
    let mut c = Circuit { title: NAMES[0].into(), ..Default::default() };
    let fg = c.add(Kind::FuncGen, GP::new(-20, -8), 0);
    {
        let p = &mut c.comp_mut(fg).unwrap().params;
        p.value = 5.0;
        p.freq = 1000.0;
    }
    // ground right on the COM pin
    c.add(Kind::Ground, GP::new(-20, -4), 0);
    let r1 = c.add(Kind::Resistor, GP::new(-8, 0), 0);
    let c1 = c.add(Kind::Capacitor, GP::new(0, 3), 1);
    c.comp_mut(c1).unwrap().params.value = 100e-9;
    c.add(Kind::Ground, GP::new(0, 6), 0);
    let sc = c.add(Kind::Oscilloscope, GP::new(8, -12), 0);
    wire(&mut c, (fg, 0), &[(-22, 0)], (r1, 0));
    wire(&mut c, (r1, 1), &[], (c1, 0));
    // scope A to the input rail, B to the output node
    let a = pin(&c, sc, 0);
    c.add_wire(vec![a, GP::new(a.x, -6), GP::new(-14, -6), GP::new(-14, 0)]);
    let b = pin(&c, sc, 1);
    c.add_wire(vec![b, GP::new(b.x, 0), GP::new(0, 0)]);
    c.add(Kind::Probe, GP::new(-3, 0), 0);
    if let Some(w) = c.wires.iter_mut().find(|w| w.points[0] == b) {
        w.color = WireColor::Blue;
    }
    c
}

pub fn divider() -> Circuit {
    let mut c = Circuit { title: NAMES[1].into(), ..Default::default() };
    let v1 = c.add(Kind::DcVoltage, GP::new(-12, 3), 0);
    c.comp_mut(v1).unwrap().params.value = 12.0;
    let am = c.add(Kind::Ammeter, GP::new(-6, 0), 0);
    let r1 = c.add(Kind::Resistor, GP::new(4, 0), 0);
    c.comp_mut(r1).unwrap().params.value = 10e3;
    let r2 = c.add(Kind::Resistor, GP::new(10, 4), 1);
    c.comp_mut(r2).unwrap().params.value = 4.7e3;
    c.add(Kind::Ground, GP::new(-12, 10), 0);
    c.add(Kind::Voltmeter, GP::new(18, 4), 1);
    c.add(Kind::Multimeter, GP::new(26, 3), 0);
    wire(&mut c, (v1, 0), &[], (am, 0));
    wire(&mut c, (am, 1), &[], (r1, 0));
    wire(&mut c, (r1, 1), &[(10, 0)], (r2, 0));
    wire_pts(&mut c, &[(10, 0), (18, 0)]);
    wire_pts(&mut c, &[(-12, 6), (-12, 10)]);
    wire_pts(&mut c, &[(10, 7), (10, 10)]);
    wire_pts(&mut c, &[(18, 8), (18, 10), (-12, 10)]);
    wire_pts(&mut c, &[(25, 7), (22, 7), (22, 0), (18, 0)]);
    wire_pts(&mut c, &[(27, 7), (27, 10), (18, 10)]);
    cleanup(&mut c);
    c
}

pub fn rectifier() -> Circuit {
    let mut c = Circuit { title: NAMES[2].into(), ..Default::default() };
    let v1 = c.add(Kind::AcVoltage, GP::new(-14, 3), 0);
    {
        let p = &mut c.comp_mut(v1).unwrap().params;
        p.value = 10.0;
        p.freq = 60.0;
    }
    let d1 = c.add(Kind::Diode, GP::new(-6, 0), 0);
    let c1 = c.add(Kind::Capacitor, GP::new(2, 3), 1);
    c.comp_mut(c1).unwrap().params.value = 47e-6;
    let r1 = c.add(Kind::Resistor, GP::new(8, 3), 1);
    let g = c.add(Kind::Ground, GP::new(-14, 6), 0);
    let _ = g;
    let sc = c.add(Kind::Oscilloscope, GP::new(18, -10), 0);
    wire(&mut c, (v1, 0), &[(-14, 0)], (d1, 0));
    wire(&mut c, (d1, 1), &[], (c1, 0));
    wire(&mut c, (c1, 0), &[], (r1, 0));
    wire(&mut c, (c1, 1), &[], (r1, 1));
    c.add_wire(vec![GP::new(2, 6), GP::new(-14, 6)]);
    let a = pin(&c, sc, 0);
    c.add_wire(vec![a, GP::new(a.x, -3), GP::new(-12, -3), GP::new(-12, 0)]);
    let b = pin(&c, sc, 1);
    c.add_wire(vec![b, GP::new(b.x, 0), GP::new(8, 0)]);
    if let Some(w) = c.wires.iter_mut().find(|w| w.points[0] == b) {
        w.color = WireColor::Blue;
    }
    cleanup(&mut c);
    c
}

pub fn led_switch() -> Circuit {
    let mut c = Circuit { title: NAMES[3].into(), ..Default::default() };
    let v1 = c.add(Kind::DcVoltage, GP::new(-16, 3), 0);
    c.comp_mut(v1).unwrap().params.value = 9.0;
    let s1 = c.add(Kind::Switch, GP::new(-6, 0), 0);
    let r1 = c.add(Kind::Resistor, GP::new(2, 0), 0);
    c.comp_mut(r1).unwrap().params.value = 470.0;
    let led = c.add(Kind::Led, GP::new(8, 3), 1);
    let am = c.add(Kind::Ammeter, GP::new(0, 6), 2);
    c.add(Kind::Ground, GP::new(-16, 6), 0);
    wire(&mut c, (v1, 0), &[(-16, 0)], (s1, 0));
    wire(&mut c, (s1, 1), &[], (r1, 0));
    wire(&mut c, (r1, 1), &[], (led, 0));
    wire(&mut c, (led, 1), &[], (am, 0));
    let a1 = pin(&c, am, 1);
    c.add_wire(vec![a1, GP::new(-16, 6)]);
    cleanup(&mut c);
    c
}

pub fn rlc() -> Circuit {
    let mut c = Circuit { title: NAMES[4].into(), ..Default::default() };
    let fg = c.add(Kind::FuncGen, GP::new(-20, -8), 0);
    {
        let p = &mut c.comp_mut(fg).unwrap().params;
        p.value = 1.0;
        p.freq = 1590.0;
    }
    c.add(Kind::Ground, GP::new(-20, -4), 0);
    let r1 = c.add(Kind::Resistor, GP::new(-10, 0), 0);
    c.comp_mut(r1).unwrap().params.value = 10.0;
    let l1 = c.add(Kind::Inductor, GP::new(-2, 0), 0);
    c.comp_mut(l1).unwrap().params.value = 10e-3;
    let c1 = c.add(Kind::Capacitor, GP::new(4, 3), 1);
    c.comp_mut(c1).unwrap().params.value = 1e-6;
    c.add(Kind::Ground, GP::new(4, 6), 0);
    let sc = c.add(Kind::Oscilloscope, GP::new(14, -12), 0);
    wire(&mut c, (fg, 0), &[(-22, 0)], (r1, 0));
    wire(&mut c, (r1, 1), &[], (l1, 0));
    wire(&mut c, (l1, 1), &[], (c1, 0));
    let a = pin(&c, sc, 0);
    c.add_wire(vec![a, GP::new(a.x, -6), GP::new(-16, -6), GP::new(-16, 0)]);
    let b = pin(&c, sc, 1);
    c.add_wire(vec![b, GP::new(b.x, 0), GP::new(4, 0)]);
    if let Some(w) = c.wires.iter_mut().find(|w| w.points[0] == b) {
        w.color = WireColor::Blue;
    }
    cleanup(&mut c);
    c
}

pub fn zener() -> Circuit {
    let mut c = Circuit { title: NAMES[5].into(), ..Default::default() };
    c.add(Kind::DcVoltage, GP::new(-12, 3), 0);
    let r1 = c.add(Kind::Resistor, GP::new(-4, 0), 0);
    c.comp_mut(r1).unwrap().params.value = 330.0;
    c.add(Kind::Zener, GP::new(4, 3), 3);
    let rl = c.add(Kind::Potentiometer, GP::new(12, 3), 1);
    c.comp_mut(rl).unwrap().params.value = 2e3;
    c.add(Kind::Voltmeter, GP::new(20, 4), 1);
    c.add(Kind::Ground, GP::new(-12, 8), 0);
    wire_pts(&mut c, &[(-12, 0), (-7, 0)]);
    wire_pts(&mut c, &[(-1, 0), (4, 0)]);
    wire_pts(&mut c, &[(4, 0), (12, 0)]);
    wire_pts(&mut c, &[(12, 0), (20, 0)]);
    wire_pts(&mut c, &[(-12, 6), (-12, 8)]);
    wire_pts(&mut c, &[(4, 6), (4, 8)]);
    wire_pts(&mut c, &[(12, 6), (12, 8)]);
    wire_pts(&mut c, &[(20, 8), (-12, 8)]);
    wire_pts(&mut c, &[(15, 3), (16, 3), (16, 0)]);
    cleanup(&mut c);
    c
}

pub fn dimmer() -> Circuit {
    let mut c = Circuit { title: NAMES[6].into(), ..Default::default() };
    let v1 = c.add(Kind::DcVoltage, GP::new(-12, 3), 0);
    let pot = c.add(Kind::Potentiometer, GP::new(-4, 0), 0);
    {
        let p = &mut c.comp_mut(pot).unwrap().params;
        p.value = 50.0;
        p.setting = 20.0;
    }
    let x1 = c.add(Kind::Lamp, GP::new(6, 3), 1);
    let am = c.add(Kind::Ammeter, GP::new(-2, 6), 2);
    c.add(Kind::Ground, GP::new(-12, 6), 0);
    wire(&mut c, (v1, 0), &[(-12, 0)], (pot, 0));
    wire(&mut c, (pot, 1), &[], (x1, 0));
    let w = pin(&c, pot, 2);
    c.add_wire(vec![w, GP::new(w.x, -5), GP::new(4, -5), GP::new(4, 0)]);
    wire(&mut c, (x1, 1), &[], (am, 0));
    let a1 = pin(&c, am, 1);
    c.add_wire(vec![a1, GP::new(-12, 6)]);
    cleanup(&mut c);
    c
}

pub fn opamp() -> Circuit {
    let mut c = Circuit { title: NAMES[7].into(), ..Default::default() };
    let fg = c.add(Kind::FuncGen, GP::new(-14, -6), 0);
    {
        let p = &mut c.comp_mut(fg).unwrap().params;
        p.value = 0.5;
        p.freq = 1000.0;
    }
    c.add(Kind::Ground, GP::new(-14, -2), 0);
    c.add(Kind::OpAmp, GP::new(10, 0), 0);
    let r1 = c.add(Kind::Resistor, GP::new(3, -2), 0);
    c.add(Kind::Ground, GP::new(0, -2), 0);
    let r2 = c.add(Kind::Resistor, GP::new(9, -8), 0);
    c.comp_mut(r2).unwrap().params.value = 10e3;
    let c1 = c.add(Kind::Capacitor, GP::new(9, -13), 0);
    c.comp_mut(c1).unwrap().params.value = 1.5e-9;
    let _ = r1;
    c.add(Kind::Oscilloscope, GP::new(24, 8), 0);
    c.add(Kind::BodePlotter, GP::new(24, -8), 0);
    c.add(Kind::FreqCounter, GP::new(34, -8), 0);
    wire_pts(&mut c, &[(-16, -2), (-16, 2), (6, 2)]);
    wire_pts(&mut c, &[(6, -2), (6, -8)]);
    wire_pts(&mut c, &[(6, -8), (6, -13)]);
    wire_pts(&mut c, &[(12, -8), (12, -13)]);
    wire_pts(&mut c, &[(12, -8), (16, -8), (16, 0), (14, 0)]);
    wire_pts(&mut c, &[(21, 12), (21, 14), (-8, 14), (-8, 2)]);
    wire_pts(&mut c, &[(23, 12), (23, 13), (29, 13), (29, 0), (16, 0)]);
    wire_pts(&mut c, &[(26, -4), (26, 0)]);
    wire_pts(&mut c, &[(22, -4), (22, -3), (19, -3), (19, -17), (-10, -17), (-10, 2)]);
    wire_pts(&mut c, &[(34, -4), (34, 0), (29, 0)]);
    if let Some(w) = c.wires.iter_mut().find(|w| w.points[0] == GP::new(23, 12)) {
        w.color = WireColor::Blue;
    }
    cleanup(&mut c);
    c
}

pub fn motor() -> Circuit {
    let mut c = Circuit { title: NAMES[8].into(), ..Default::default() };
    c.add(Kind::DcVoltage, GP::new(-12, 3), 0);
    c.add(Kind::Ground, GP::new(-12, 6), 0);
    let f1 = c.add(Kind::Fuse, GP::new(-6, 0), 0);
    c.comp_mut(f1).unwrap().params.rating = 2.0;
    c.add(Kind::SpdtSwitch, GP::new(2, 0), 0);
    let m1 = c.add(Kind::Motor, GP::new(12, -2), 0);
    c.comp_mut(m1).unwrap().params.rating = 1.0;
    c.add(Kind::Buzzer, GP::new(12, 6), 0);
    c.add(Kind::Ground, GP::new(15, -2), 0);
    c.add(Kind::Ground, GP::new(15, 6), 0);
    wire_pts(&mut c, &[(-12, 0), (-9, 0)]);
    wire_pts(&mut c, &[(-3, 0), (-1, 0)]);
    wire_pts(&mut c, &[(5, -2), (9, -2)]);
    wire_pts(&mut c, &[(5, 2), (7, 2), (7, 6), (9, 6)]);
    cleanup(&mut c);
    c
}

/// Drop zero-length wires produced by pins that already touch.
fn cleanup(c: &mut Circuit) {
    c.wires.retain(|w| w.points.len() >= 2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::*;

    #[test]
    fn examples_simulate() {
        for i in 0..NAMES.len() {
            let c = by_index(i);
            let nl = build_netlist(&c);
            // every non-optional pin should touch something else
            for (ci, comp) in c.components.iter().enumerate() {
                for (k, n) in nl.comp_nets[ci].iter().enumerate() {
                    let optional = matches!(comp.kind, Kind::Oscilloscope) && k >= 2 || matches!(comp.kind, Kind::FuncGen) && k == 2 || matches!(comp.kind, Kind::Probe);
                    if !optional && *n != 0 {
                        assert!(nl.net_pin_count[*n] > 1, "{}: {} pin {} unconnected", NAMES[i], comp.refdes, k);
                    }
                }
            }
            let mut s = Sim::new(&c).unwrap();
            s.dc_op().unwrap_or_else(|e| panic!("{}: {}", NAMES[i], e));
            for _ in 0..2000 {
                s.step(1e-5).unwrap_or_else(|e| panic!("{}: {}", NAMES[i], e));
            }
            let mut line = format!("{}:", NAMES[i]);
            for n in 1..nl.n_nets {
                line += &format!(" V{}={:.3}", n, s.v(n));
            }
            println!("{}", line);
        }
    }
}
