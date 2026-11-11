//! Breadboard rules: hole layout, which holes are joined, and turning a board into a netlist.

use crate::model::*;
use crate::sim::Netlist;
use std::collections::HashMap;

pub const COLS: i32 = 30;

/// Parts that can be plugged into the breadboard.
pub const BOARD_PARTS: &[(&str, &[Kind])] = &[
    ("Power", &[Kind::DcVoltage, Kind::ClockSource]),
    ("Passive", &[Kind::Resistor, Kind::Capacitor, Kind::Inductor, Kind::Potentiometer]),
    ("Semiconductors", &[Kind::Led, Kind::Diode, Kind::Zener, Kind::OpAmp]),
    ("Switches", &[Kind::PushButton, Kind::Switch]),
    ("Outputs", &[Kind::Lamp, Kind::Buzzer, Kind::Motor]),
    ("Measure", &[Kind::Multimeter]),
];

pub fn is_hole(h: BH) -> bool {
    (0..COLS).contains(&h.x) && matches!(h.y, 0 | 1 | 3..=7 | 9..=13 | 15 | 16)
}

pub fn is_rail(y: i32) -> bool {
    matches!(y, 0 | 1 | 15 | 16)
}

/// Rail rows: 0 and 16 are "−" (blue), 1 and 15 are "+" (red).
pub fn rail_is_plus(y: i32) -> bool {
    y == 1 || y == 15
}

/// The conductor a hole belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Node {
    Rail(i32),
    Strip(i32, bool),
}

pub fn node(h: BH) -> Node {
    if is_rail(h.y) {
        Node::Rail(h.y)
    } else {
        Node::Strip(h.x, h.y <= 7)
    }
}

/// All holes that share a conductor with `h` (for highlighting).
pub fn strip_holes(h: BH) -> Vec<BH> {
    if is_rail(h.y) {
        (0..COLS).map(|x| BH::new(x, h.y)).collect()
    } else if h.y <= 7 {
        (3..=7).map(|y| BH::new(h.x, y)).collect()
    } else {
        (9..=13).map(|y| BH::new(h.x, y)).collect()
    }
}

pub fn row_label(y: i32) -> &'static str {
    match y {
        3 => "a",
        4 => "b",
        5 => "c",
        6 => "d",
        7 => "e",
        9 => "f",
        10 => "g",
        11 => "h",
        12 => "i",
        13 => "j",
        _ => "",
    }
}

/// Parts whose body sits off the board, joined by two flying leads.
pub fn is_flying(k: Kind) -> bool {
    matches!(k, Kind::DcVoltage | Kind::ClockSource | Kind::Motor | Kind::Multimeter)
}

/// Pin holes for parts placed with a single click (3-pin parts and chips).
pub fn auto_pins(k: Kind, at: BH) -> Option<Vec<BH>> {
    match k {
        // pins in Kind order: 1, 2, wiper
        Kind::Potentiometer => Some(vec![at, BH::new(at.x + 2, at.y), BH::new(at.x + 1, at.y)]),
        // 8-pin chip straddling the centre channel: pin2 (−) and pin3 (+) on row f, pin6 (out) on row e
        Kind::OpAmp => Some(vec![BH::new(at.x + 1, 9), BH::new(at.x + 2, 9), BH::new(at.x + 2, 7)]),
        _ => None,
    }
}

/// Every hole an op-amp chip covers (for drawing and occupancy).
pub fn chip_holes(p: &BoardPart) -> Vec<BH> {
    if p.comp.kind != Kind::OpAmp {
        return p.pins.clone();
    }
    let x0 = p.pins[0].x - 1;
    let mut v = Vec::new();
    for dx in 0..4 {
        v.push(BH::new(x0 + dx, 7));
        v.push(BH::new(x0 + dx, 9));
    }
    v
}

/// Holes already in use by part leads and jumper ends.
pub fn occupied(b: &Board, skip_part: Option<u64>, skip_jumper: Option<u64>) -> std::collections::HashSet<BH> {
    let mut s = std::collections::HashSet::new();
    for p in &b.parts {
        if Some(p.comp.id) != skip_part {
            s.extend(chip_holes(p));
        }
    }
    for j in &b.jumpers {
        if Some(j.id) != skip_jumper {
            s.insert(j.a);
            s.insert(j.b);
        }
    }
    s
}

pub fn next_refdes(b: &Board, kind: Kind) -> String {
    let prefix = kind.prefix();
    let mut max = 0;
    for p in &b.parts {
        if let Some(rest) = p.comp.refdes.strip_prefix(prefix) {
            if let Ok(n) = rest.parse::<u32>() {
                max = max.max(n);
            }
        }
    }
    format!("{}{}", prefix, max + 1)
}

/// A circuit holding the board's parts, plus the netlist made by the board's strips and jumpers.
pub fn sim_circuit(b: &Board) -> (Circuit, Netlist) {
    let c = Circuit { title: "Breadboard".into(), components: b.parts.iter().map(|p| p.comp.clone()).collect(), ..Default::default() };
    let mut ids: HashMap<Node, usize> = HashMap::new();
    let mut parent: Vec<usize> = Vec::new();
    let mut id_of = |n: Node, parent: &mut Vec<usize>| -> usize {
        *ids.entry(n).or_insert_with(|| {
            parent.push(parent.len());
            parent.len() - 1
        })
    };
    fn find(p: &mut Vec<usize>, x: usize) -> usize {
        let mut r = x;
        while p[r] != r {
            r = p[r];
        }
        let mut c = x;
        while p[c] != r {
            let n = p[c];
            p[c] = r;
            c = n;
        }
        r
    }
    let mut pin_ids: Vec<Vec<usize>> = Vec::new();
    for part in &b.parts {
        pin_ids.push(part.pins.iter().map(|h| id_of(node(*h), &mut parent)).collect());
    }
    let jumper_ids: Vec<(usize, usize)> = b.jumpers.iter().map(|j| (id_of(node(j.a), &mut parent), id_of(node(j.b), &mut parent))).collect();
    for (a, bb) in jumper_ids {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, bb));
        if ra != rb {
            parent[rb] = ra;
        }
    }
    // ground: the battery's − lead, else a signal source's −, else the top − rail
    let mut ground: Option<usize> = None;
    for kind in [Kind::DcVoltage, Kind::ClockSource] {
        if ground.is_none() {
            if let Some(i) = b.parts.iter().position(|p| p.comp.kind == kind) {
                ground = Some(find(&mut parent, pin_ids[i][1]));
            }
        }
    }
    if ground.is_none() {
        if let Some(&g) = ids.get(&Node::Rail(0)) {
            ground = Some(find(&mut parent, g));
        }
    }
    let mut root_to_net: HashMap<usize, usize> = HashMap::new();
    if let Some(g) = ground {
        root_to_net.insert(g, 0);
    }
    let mut next = 1;
    let mut comp_nets = Vec::new();
    for pins in &pin_ids {
        let mut v = Vec::new();
        for &pid in pins {
            let r = find(&mut parent, pid);
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
    let nl = Netlist { n_nets: next, comp_nets, net_pin_count, wire_nets: Vec::new(), has_ground: ground.is_some() };
    (c, nl)
}

fn part(c: &mut Circuit, kind: Kind, pins: &[(i32, i32)], body: Option<(f32, f32)>) -> usize {
    let id = c.new_id();
    let refdes = next_refdes(&c.board, kind);
    c.board.parts.push(BoardPart {
        comp: Component { id, kind, refdes, pos: GP::new(0, 0), rot: 0, params: kind.default_params() },
        pins: pins.iter().map(|&(x, y)| BH::new(x, y)).collect(),
        body,
    });
    c.board.parts.len() - 1
}

fn jumper(c: &mut Circuit, a: (i32, i32), b: (i32, i32), color: WireColor) {
    let id = c.new_id();
    c.board.jumpers.push(Jumper { id, a: BH::new(a.0, a.1), b: BH::new(b.0, b.1), color });
}

/// A ready-made board: button-lit LED, blinking LED, dimmer with a meter, and a switched buzzer.
pub fn starter_board(c: &mut Circuit) {
    c.board = Board::default();
    let bat = part(c, Kind::DcVoltage, &[(1, 1), (1, 0)], Some((-5.5, 0.5)));
    c.board.parts[bat].comp.params.value = 9.0;
    jumper(c, (27, 1), (27, 15), WireColor::Red);
    jumper(c, (28, 0), (28, 16), WireColor::Black);
    // 1) push the button to light the red LED
    jumper(c, (3, 1), (3, 3), WireColor::Red);
    part(c, Kind::PushButton, &[(3, 5), (6, 5)], None);
    let r1 = part(c, Kind::Resistor, &[(6, 4), (10, 4)], None);
    c.board.parts[r1].comp.params.value = 470.0;
    part(c, Kind::Led, &[(10, 6), (11, 6)], None);
    jumper(c, (11, 3), (11, 0), WireColor::Black);
    // 2) a clock source blinks the green LED
    let clk = part(c, Kind::ClockSource, &[(4, 13), (5, 16)], Some((4.5, 21.5)));
    c.board.parts[clk].comp.params.freq = 2.0;
    let r2 = part(c, Kind::Resistor, &[(4, 11), (8, 11)], None);
    c.board.parts[r2].comp.params.value = 330.0;
    let led2 = part(c, Kind::Led, &[(8, 10), (9, 10)], None);
    c.board.parts[led2].comp.params.led = LedColor::Green;
    jumper(c, (9, 13), (9, 16), WireColor::Black);
    // 3) a potentiometer dims the yellow LED; the multimeter reads its voltage
    let pot = part(c, Kind::Potentiometer, &[(16, 4), (18, 4), (17, 4)], None);
    c.board.parts[pot].comp.params.value = 1000.0;
    jumper(c, (16, 3), (16, 1), WireColor::Red);
    jumper(c, (18, 3), (18, 0), WireColor::Black);
    let r3 = part(c, Kind::Resistor, &[(17, 6), (21, 6)], None);
    c.board.parts[r3].comp.params.value = 220.0;
    let led3 = part(c, Kind::Led, &[(21, 7), (22, 7)], None);
    c.board.parts[led3].comp.params.led = LedColor::Yellow;
    jumper(c, (22, 3), (22, 0), WireColor::Black);
    part(c, Kind::Multimeter, &[(21, 5), (23, 0)], Some((21.0, -7.0)));
    // 4) the slide switch sounds the buzzer
    jumper(c, (13, 15), (13, 9), WireColor::Orange);
    part(c, Kind::Switch, &[(13, 11), (15, 11)], None);
    part(c, Kind::Buzzer, &[(15, 12), (18, 12)], None);
    jumper(c, (18, 13), (18, 16), WireColor::Black);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Sim;

    #[test]
    fn starter_board_runs() {
        let mut c = Circuit::default();
        starter_board(&mut c);
        let (bc, nl) = sim_circuit(&c.board);
        assert!(nl.has_ground);
        let mut s = Sim::with_netlist(&bc, nl).unwrap();
        for _ in 0..500 {
            s.step(1e-4).unwrap();
        }
        // yellow LED branch conducts (pot at 50 %)
        let i = bc.components.iter().position(|p| p.kind == Kind::Led && p.params.led == LedColor::Yellow).unwrap();
        assert!(s.comp_current(i) > 1e-3, "{}", s.comp_current(i));
        // red LED is dark while the button is up
        let r = bc.components.iter().position(|p| p.kind == Kind::Led && p.params.led == LedColor::Red).unwrap();
        assert!(s.comp_current(r).abs() < 1e-6);
    }
}
