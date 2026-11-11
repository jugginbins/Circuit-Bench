//! Circuit data model: parts, wires, values and geometry.

use serde::{Deserialize, Serialize};

/// A point on the schematic grid (integer grid units).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct GP {
    pub x: i32,
    pub y: i32,
}

impl GP {
    pub const fn new(x: i32, y: i32) -> Self {
        GP { x, y }
    }
    pub fn add(self, o: GP) -> GP {
        GP::new(self.x + o.x, self.y + o.y)
    }
    pub fn sub(self, o: GP) -> GP {
        GP::new(self.x - o.x, self.y - o.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Resistor,
    Capacitor,
    Inductor,
    Potentiometer,
    DcVoltage,
    DcCurrent,
    AcVoltage,
    Ground,
    Diode,
    Zener,
    Led,
    Switch,
    Lamp,
    Voltmeter,
    Ammeter,
    Probe,
    Multimeter,
    FuncGen,
    Oscilloscope,
    ClockSource,
    AcCurrent,
    PushButton,
    SpdtSwitch,
    Fuse,
    OpAmp,
    Motor,
    Buzzer,
    Wattmeter,
    BodePlotter,
    FreqCounter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wave {
    Sine,
    Triangle,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeterMode {
    Volts,
    Amps,
    Ohms,
    Decibel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedColor {
    Red,
    Green,
    Yellow,
    Blue,
    White,
    Orange,
    Purple,
}

impl LedColor {
    pub fn forward_voltage(self) -> f64 {
        match self {
            LedColor::Red => 1.8,
            LedColor::Yellow => 2.0,
            LedColor::Green => 2.1,
            LedColor::Blue => 3.0,
            LedColor::White => 3.1,
            LedColor::Orange => 1.9,
            LedColor::Purple => 3.2,
        }
    }
    pub fn rgb(self) -> [u8; 3] {
        match self {
            LedColor::Red => [255, 40, 30],
            LedColor::Yellow => [255, 220, 0],
            LedColor::Green => [40, 230, 60],
            LedColor::Blue => [40, 110, 255],
            LedColor::White => [250, 250, 250],
            LedColor::Orange => [255, 140, 20],
            LedColor::Purple => [190, 70, 255],
        }
    }
    pub const ALL: [LedColor; 7] = [
        LedColor::Red,
        LedColor::Orange,
        LedColor::Yellow,
        LedColor::Green,
        LedColor::Blue,
        LedColor::Purple,
        LedColor::White,
    ];
    pub fn name(self) -> &'static str {
        match self {
            LedColor::Red => "Red",
            LedColor::Green => "Green",
            LedColor::Yellow => "Yellow",
            LedColor::Blue => "Blue",
            LedColor::White => "White",
            LedColor::Orange => "Orange",
            LedColor::Purple => "Purple",
        }
    }
}

/// Every adjustable value of a part. Unused fields are ignored by a kind.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    /// Main value: ohms, farads, henries, volts, amps, Zener voltage, lamp volts.
    pub value: f64,
    /// Frequency (AC source, function generator).
    pub freq: f64,
    /// Phase in degrees (AC source).
    pub phase: f64,
    /// DC offset (AC source, function generator).
    pub offset: f64,
    /// Duty cycle in % (function generator square).
    pub duty: f64,
    /// Lamp rated power (W).
    pub power: f64,
    /// Potentiometer setting 0..100 %.
    pub setting: f64,
    /// Potentiometer increment %.
    pub increment: f64,
    /// Interactive key for switches / potentiometers.
    pub key: char,
    /// Switch closed state.
    pub closed: bool,
    pub wave: Wave,
    pub meter: MeterMode,
    /// AC (true) or DC (false) reading for meters.
    pub ac: bool,
    pub led: LedColor,
    /// Diode saturation current.
    pub is: f64,
    /// Diode emission coefficient.
    pub n: f64,
    /// Initial capacitor voltage / inductor current.
    pub ic: f64,
    /// Rating: fuse current, LED max current, motor/buzzer rated current (A).
    pub rating: f64,
    /// Internal resistance: meters, switch on-resistance, source series resistance.
    pub r_internal: f64,
    /// Multimeter ammeter resistance (0 = ideal).
    pub r_amm: f64,
    /// Multimeter ohmmeter test current.
    pub i_test: f64,
    /// LED forward voltage override (0 = from colour).
    pub vf: f64,
    /// Fuse state.
    pub blown: bool,
    /// Push button: normally closed instead of normally open.
    pub normally_closed: bool,
    /// Show the value text on the schematic.
    pub show_value: bool,
    /// Stop frequency (Bode plotter).
    pub f_stop: f64,
    /// Diode reverse breakdown voltage (0 = none).
    pub bv: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            value: 1.0,
            freq: 1000.0,
            phase: 0.0,
            offset: 0.0,
            duty: 50.0,
            power: 1.0,
            setting: 50.0,
            increment: 5.0,
            key: 'A',
            closed: true,
            wave: Wave::Sine,
            meter: MeterMode::Volts,
            ac: false,
            led: LedColor::Red,
            is: 2.52e-9,
            n: 1.752,
            ic: 0.0,
            rating: 1.0,
            r_internal: 0.0,
            r_amm: 0.0,
            i_test: 1e-3,
            vf: 0.0,
            blown: false,
            normally_closed: false,
            show_value: true,
            f_stop: 1e6,
            bv: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Component {
    pub id: u64,
    pub kind: Kind,
    pub refdes: String,
    pub pos: GP,
    /// Rotation in quarter turns, clockwise.
    pub rot: u8,
    pub params: Params,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireColor {
    Red,
    Blue,
    Green,
    Black,
    Orange,
    Purple,
    Cyan,
    Pink,
}

impl WireColor {
    pub const ALL: [WireColor; 8] = [
        WireColor::Cyan,
        WireColor::Pink,
        WireColor::Red,
        WireColor::Blue,
        WireColor::Green,
        WireColor::Black,
        WireColor::Orange,
        WireColor::Purple,
    ];
    pub fn rgb(self) -> [u8; 3] {
        match self {
            WireColor::Red => [210, 30, 30],
            WireColor::Blue => [30, 80, 220],
            WireColor::Green => [20, 150, 50],
            WireColor::Black => [30, 30, 30],
            WireColor::Orange => [235, 130, 0],
            WireColor::Purple => [140, 50, 180],
            WireColor::Cyan => [0, 160, 190],
            WireColor::Pink => [225, 60, 150],
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            WireColor::Red => "Red",
            WireColor::Blue => "Blue",
            WireColor::Green => "Green",
            WireColor::Black => "Black",
            WireColor::Orange => "Orange",
            WireColor::Purple => "Purple",
            WireColor::Cyan => "Cyan",
            WireColor::Pink => "Pink",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wire {
    pub id: u64,
    pub points: Vec<GP>,
    pub color: WireColor,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Circuit {
    pub title: String,
    pub components: Vec<Component>,
    pub wires: Vec<Wire>,
    pub next_id: u64,
    /// The breadboard build of this project (its own parts and jumper wires).
    #[serde(default)]
    pub board: Board,
}

/// A hole on the breadboard: x = column 0.., y = row (0/1 top rails, 3..7 a–e, 9..13 f–j, 15/16 bottom rails).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct BH {
    pub x: i32,
    pub y: i32,
}

impl BH {
    pub const fn new(x: i32, y: i32) -> Self {
        BH { x, y }
    }
}

/// A real part plugged into the breadboard.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoardPart {
    pub comp: Component,
    /// Hole for each pin, in the same order as `Kind::pins()`.
    pub pins: Vec<BH>,
    /// Body position for parts that sit off the board on flying leads.
    pub body: Option<(f32, f32)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Jumper {
    pub id: u64,
    pub a: BH,
    pub b: BH,
    pub color: WireColor,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Board {
    pub parts: Vec<BoardPart>,
    pub jumpers: Vec<Jumper>,
}

impl Circuit {
    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn next_refdes(&self, kind: Kind) -> String {
        let prefix = kind.prefix();
        if prefix.is_empty() {
            return String::new();
        }
        let mut max = 0;
        for c in &self.components {
            if let Some(rest) = c.refdes.strip_prefix(prefix) {
                if let Ok(n) = rest.parse::<u32>() {
                    max = max.max(n);
                }
            }
        }
        format!("{}{}", prefix, max + 1)
    }

    pub fn add(&mut self, kind: Kind, pos: GP, rot: u8) -> u64 {
        let id = self.new_id();
        let refdes = self.next_refdes(kind);
        self.components.push(Component {
            id,
            kind,
            refdes,
            pos,
            rot,
            params: kind.default_params(),
        });
        id
    }

    pub fn add_wire(&mut self, points: Vec<GP>) -> u64 {
        let id = self.new_id();
        let points = simplify(points);
        self.wires.push(Wire {
            id,
            points,
            color: WireColor::Red,
        });
        id
    }

    pub fn comp(&self, id: u64) -> Option<&Component> {
        self.components.iter().find(|c| c.id == id)
    }
    pub fn comp_mut(&mut self, id: u64) -> Option<&mut Component> {
        self.components.iter_mut().find(|c| c.id == id)
    }
    /// A part from either the schematic or the breadboard.
    pub fn comp_any(&self, id: u64) -> Option<&Component> {
        self.components.iter().find(|c| c.id == id).or_else(|| self.board.parts.iter().map(|p| &p.comp).find(|c| c.id == id))
    }
    pub fn comp_any_mut(&mut self, id: u64) -> Option<&mut Component> {
        if let Some(i) = self.components.iter().position(|c| c.id == id) {
            return Some(&mut self.components[i]);
        }
        self.board.parts.iter_mut().map(|p| &mut p.comp).find(|c| c.id == id)
    }
    /// Every part, schematic and breadboard.
    pub fn all_comps_mut(&mut self) -> impl Iterator<Item = &mut Component> {
        self.components.iter_mut().chain(self.board.parts.iter_mut().map(|p| &mut p.comp))
    }
}

/// Remove duplicate and collinear points from a polyline.
pub fn simplify(points: Vec<GP>) -> Vec<GP> {
    let mut out: Vec<GP> = Vec::new();
    for p in points {
        if out.last() == Some(&p) {
            continue;
        }
        if out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let collinear = (a.x == b.x && b.x == p.x) || (a.y == b.y && b.y == p.y);
            if collinear {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

impl Kind {
    pub fn prefix(self) -> &'static str {
        match self {
            Kind::Resistor | Kind::Potentiometer => "R",
            Kind::Capacitor => "C",
            Kind::Inductor => "L",
            Kind::DcVoltage | Kind::AcVoltage => "V",
            Kind::DcCurrent => "I",
            Kind::Ground => "",
            Kind::Diode | Kind::Zener => "D",
            Kind::Led => "LED",
            Kind::Switch => "S",
            Kind::Lamp => "X",
            Kind::Voltmeter | Kind::Ammeter => "U",
            Kind::Probe => "Probe",
            Kind::Multimeter => "XMM",
            Kind::FuncGen => "XFG",
            Kind::Oscilloscope => "XSC",
            Kind::ClockSource => "V",
            Kind::AcCurrent => "I",
            Kind::PushButton | Kind::SpdtSwitch => "S",
            Kind::Fuse => "F",
            Kind::OpAmp => "OA",
            Kind::Motor => "M",
            Kind::Buzzer => "BZ",
            Kind::Wattmeter => "XWM",
            Kind::BodePlotter => "XBP",
            Kind::FreqCounter => "XFC",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Resistor => "Resistor",
            Kind::Capacitor => "Capacitor",
            Kind::Inductor => "Inductor",
            Kind::Potentiometer => "Potentiometer",
            Kind::DcVoltage => "DC voltage source",
            Kind::DcCurrent => "DC current source",
            Kind::AcVoltage => "AC voltage source",
            Kind::Ground => "Ground",
            Kind::Diode => "Diode",
            Kind::Zener => "Zener diode",
            Kind::Led => "LED",
            Kind::Switch => "Switch (SPST)",
            Kind::Lamp => "Lamp",
            Kind::Voltmeter => "Voltmeter",
            Kind::Ammeter => "Ammeter",
            Kind::Probe => "Voltage probe",
            Kind::Multimeter => "Multimeter",
            Kind::FuncGen => "Function generator",
            Kind::Oscilloscope => "Oscilloscope",
            Kind::ClockSource => "Clock (square) source",
            Kind::AcCurrent => "AC current source",
            Kind::PushButton => "Push button",
            Kind::SpdtSwitch => "Switch (SPDT)",
            Kind::Fuse => "Fuse",
            Kind::OpAmp => "Op-amp",
            Kind::Motor => "DC motor",
            Kind::Buzzer => "Buzzer",
            Kind::Wattmeter => "Wattmeter",
            Kind::BodePlotter => "Bode plotter",
            Kind::FreqCounter => "Frequency counter",
        }
    }

    pub fn is_instrument(self) -> bool {
        matches!(self, Kind::Multimeter | Kind::FuncGen | Kind::Oscilloscope | Kind::Wattmeter | Kind::BodePlotter | Kind::FreqCounter)
    }

    pub fn default_params(self) -> Params {
        let mut p = Params::default();
        match self {
            Kind::Resistor => p.value = 1000.0,
            Kind::Capacitor => p.value = 1e-6,
            Kind::Inductor => p.value = 1e-3,
            Kind::Potentiometer => {
                p.value = 10_000.0;
                p.key = 'A';
            }
            Kind::DcVoltage => p.value = 12.0,
            Kind::DcCurrent => p.value = 1e-3,
            Kind::AcVoltage => {
                p.value = 1.0;
                p.freq = 1000.0;
            }
            Kind::Diode => {
                p.is = 2.52e-9;
                p.n = 1.752;
            }
            Kind::Zener => {
                p.value = 5.1;
                p.is = 1e-14;
                p.n = 1.0;
            }
            Kind::Led => {
                p.led = LedColor::Red;
                p.rating = 0.02;
            }
            Kind::Switch => {
                p.key = ' ';
                p.closed = false;
                p.r_internal = 1e-3;
            }
            Kind::Lamp => {
                p.value = 12.0;
                p.power = 10.0;
            }
            Kind::FuncGen => {
                p.value = 10.0;
                p.freq = 1000.0;
            }
            Kind::Voltmeter => p.r_internal = 10e6,
            Kind::Ammeter => p.r_internal = 0.0,
            Kind::Multimeter => {
                p.r_internal = 1e9;
                p.r_amm = 0.0;
                p.i_test = 1e-3;
            }
            Kind::ClockSource => {
                p.value = 5.0;
                p.offset = 0.0;
                p.freq = 1000.0;
                p.duty = 50.0;
            }
            Kind::AcCurrent => {
                p.value = 1e-3;
                p.freq = 1000.0;
            }
            Kind::PushButton => {
                p.key = 'B';
                p.closed = false;
                p.r_internal = 1e-3;
            }
            Kind::SpdtSwitch => {
                p.key = 'S';
                p.closed = false;
                p.r_internal = 1e-3;
            }
            Kind::Fuse => p.rating = 0.5,
            Kind::OpAmp => {
                p.value = 2e5;
                p.power = 12.0;
            }
            Kind::Motor => {
                p.value = 10.0;
                p.rating = 0.5;
            }
            Kind::Buzzer => {
                p.value = 100.0;
                p.rating = 0.05;
                p.freq = 2000.0;
            }
            Kind::BodePlotter => {
                p.freq = 1.0;
                p.f_stop = 1e6;
            }
            Kind::FreqCounter => p.value = 0.0,
            _ => {}
        }
        p
    }

    /// Pin positions in local (unrotated) grid units, with pin names.
    pub fn pins(self) -> &'static [(i32, i32, &'static str)] {
        match self {
            Kind::Resistor
            | Kind::Capacitor
            | Kind::Inductor
            | Kind::Diode
            | Kind::Zener
            | Kind::Led
            | Kind::Switch
            | Kind::Lamp
            | Kind::PushButton
            | Kind::Fuse
            | Kind::Motor
            | Kind::Buzzer => &[(-3, 0, "1"), (3, 0, "2")],
            Kind::SpdtSwitch => &[(-3, 0, "C"), (3, -2, "A"), (3, 2, "B")],
            Kind::OpAmp => &[(-4, -2, "-"), (-4, 2, "+"), (4, 0, "OUT")],
            Kind::ClockSource | Kind::AcCurrent => &[(0, -3, "+"), (0, 3, "-")],
            Kind::Wattmeter => &[(-3, 4, "V+"), (-1, 4, "V-"), (1, 4, "I+"), (3, 4, "I-")],
            Kind::BodePlotter => &[(-2, 4, "IN"), (2, 4, "OUT")],
            Kind::FreqCounter => &[(0, 4, "IN")],
            Kind::Voltmeter | Kind::Ammeter => &[(-4, 0, "+"), (4, 0, "-")],
            Kind::Potentiometer => &[(-3, 0, "1"), (3, 0, "2"), (0, -3, "W")],
            Kind::DcVoltage | Kind::DcCurrent | Kind::AcVoltage => &[(0, -3, "+"), (0, 3, "-")],
            Kind::Ground => &[(0, 0, "GND")],
            Kind::Probe => &[(0, 0, "P")],
            Kind::Multimeter => &[(-1, 4, "+"), (1, 4, "-")],
            Kind::FuncGen => &[(-2, 4, "+"), (0, 4, "COM"), (2, 4, "-")],
            Kind::Oscilloscope => &[(-3, 4, "A"), (-1, 4, "B"), (1, 4, "Ext"), (3, 4, "G")],
        }
    }

    /// Local bounding box (min x, min y, max x, max y) used for hit tests.
    pub fn bounds(self) -> (f32, f32, f32, f32) {
        match self {
            Kind::Resistor
            | Kind::Capacitor
            | Kind::Inductor
            | Kind::Diode
            | Kind::Zener
            | Kind::Switch => (-3.0, -1.2, 3.0, 1.2),
            Kind::Led => (-3.0, -2.0, 3.0, 1.2),
            Kind::Lamp => (-3.0, -1.5, 3.0, 1.5),
            Kind::Voltmeter | Kind::Ammeter => (-4.0, -2.0, 4.0, 2.0),
            Kind::Potentiometer => (-3.0, -3.0, 3.0, 1.2),
            Kind::DcVoltage | Kind::DcCurrent | Kind::AcVoltage => (-1.6, -3.0, 1.6, 3.0),
            Kind::Ground => (-1.5, 0.0, 1.5, 2.2),
            Kind::Probe => (-0.5, -3.2, 5.6, 0.5),
            Kind::Multimeter => (-2.0, -3.0, 2.0, 4.0),
            Kind::FuncGen => (-3.0, -3.0, 3.0, 4.0),
            Kind::Oscilloscope => (-4.0, -3.0, 4.0, 4.0),
            Kind::ClockSource | Kind::AcCurrent => (-1.6, -3.0, 1.6, 3.0),
            Kind::PushButton => (-3.0, -1.8, 3.0, 0.6),
            Kind::SpdtSwitch => (-3.0, -2.4, 3.0, 2.4),
            Kind::Fuse => (-3.0, -0.9, 3.0, 0.9),
            Kind::OpAmp => (-4.0, -3.2, 4.0, 3.2),
            Kind::Motor => (-3.0, -1.7, 3.0, 1.7),
            Kind::Buzzer => (-3.0, -2.2, 3.0, 0.8),
            Kind::Wattmeter => (-4.0, -3.0, 4.0, 4.0),
            Kind::BodePlotter => (-3.0, -3.0, 3.0, 4.0),
            Kind::FreqCounter => (-2.5, -3.0, 2.5, 4.0),
        }
    }
}

/// Rotate a local point by quarter turns (clockwise, screen coordinates: y down).
pub fn rotate_i(x: i32, y: i32, rot: u8) -> (i32, i32) {
    match rot % 4 {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    }
}

pub fn rotate_f(x: f32, y: f32, rot: u8) -> (f32, f32) {
    match rot % 4 {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    }
}

impl Component {
    pub fn pin_positions(&self) -> Vec<GP> {
        self.kind
            .pins()
            .iter()
            .map(|&(x, y, _)| {
                let (rx, ry) = rotate_i(x, y, self.rot);
                GP::new(self.pos.x + rx, self.pos.y + ry)
            })
            .collect()
    }

    /// Short text shown next to the symbol.
    pub fn value_label(&self) -> String {
        let p = &self.params;
        if !p.show_value {
            return String::new();
        }
        match self.kind {
            Kind::ClockSource => format!("{}–{} {}", fmt_si(p.offset, "V"), fmt_si(p.value, "V"), fmt_si(p.freq, "Hz")),
            Kind::AcCurrent => format!("{}pk {}", fmt_si(p.value, "A"), fmt_si(p.freq, "Hz")),
            Kind::PushButton => format!("Key = {}", key_name(p.key)),
            Kind::SpdtSwitch => format!("Key = {}", key_name(p.key)),
            Kind::Fuse => if p.blown { "BLOWN".into() } else { fmt_si(p.rating, "A") },
            Kind::OpAmp => format!("A={} ±{}", fmt_si(p.value, ""), fmt_si(p.power, "V")),
            Kind::Motor => format!("{} {}", fmt_si(p.value, "Ω"), fmt_si(p.rating, "A")),
            Kind::Buzzer => fmt_si(p.freq, "Hz"),
            Kind::Resistor => fmt_si(p.value, "Ω"),
            Kind::Capacitor => fmt_si(p.value, "F"),
            Kind::Inductor => fmt_si(p.value, "H"),
            Kind::Potentiometer => format!("{} {}%  Key={}", fmt_si(p.value, "Ω"), p.setting.round(), key_name(p.key)),
            Kind::DcVoltage => fmt_si(p.value, "V"),
            Kind::DcCurrent => fmt_si(p.value, "A"),
            Kind::AcVoltage => format!("{}pk {}", fmt_si(p.value, "V"), fmt_si(p.freq, "Hz")),
            Kind::Zener => format!("BV={}", fmt_si(p.value, "V")),
            Kind::Led => p.led.name().to_string(),
            Kind::Diode => format!("Is={} N={:.2}", fmt_si(p.is, "A"), p.n),
            Kind::Switch => format!("Key = {}", key_name(p.key)),
            Kind::Lamp => format!("{} {}", fmt_si(p.value, "V"), fmt_si(p.power, "W")),
            Kind::Voltmeter | Kind::Ammeter => if p.ac { "AC".into() } else { "DC".into() },
            _ => String::new(),
        }
    }
}

pub fn key_name(c: char) -> String {
    if c == ' ' {
        "Space".to_string()
    } else {
        c.to_string()
    }
}

/// Format a value with an SI prefix: 4700 -> "4.7 kΩ".
pub fn fmt_si(v: f64, unit: &str) -> String {
    if !v.is_finite() {
        return format!("--- {}", unit);
    }
    if v == 0.0 {
        return format!("0 {}", unit);
    }
    let a = v.abs();
    let (scale, pre) = if a >= 1e12 {
        (1e12, "T")
    } else if a >= 1e9 {
        (1e9, "G")
    } else if a >= 1e6 {
        (1e6, "M")
    } else if a >= 1e3 {
        (1e3, "k")
    } else if a >= 1.0 {
        (1.0, "")
    } else if a >= 1e-3 {
        (1e-3, "m")
    } else if a >= 1e-6 {
        (1e-6, "µ")
    } else if a >= 1e-9 {
        (1e-9, "n")
    } else if a >= 1e-12 {
        (1e-12, "p")
    } else {
        (1e-15, "f")
    };
    let s = v / scale;
    let txt = if s.abs() >= 100.0 {
        format!("{:.1}", s)
    } else if s.abs() >= 10.0 {
        format!("{:.2}", s)
    } else {
        format!("{:.3}", s)
    };
    let txt = if txt.contains('.') {
        txt.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        txt
    };
    format!("{} {}{}", txt, pre, unit)
}

/// Fixed-width reading for meters: "12.003 V".
pub fn fmt_reading(v: f64, unit: &str) -> String {
    if !v.is_finite() {
        return format!("OL {}", unit);
    }
    let a = v.abs();
    let (scale, pre) = if a >= 1e9 {
        (1e9, "G")
    } else if a >= 1e6 {
        (1e6, "M")
    } else if a >= 1e3 {
        (1e3, "k")
    } else if a >= 1.0 || a == 0.0 {
        (1.0, "")
    } else if a >= 1e-3 {
        (1e-3, "m")
    } else if a >= 1e-6 {
        (1e-6, "µ")
    } else if a >= 1e-9 {
        (1e-9, "n")
    } else {
        (1e-12, "p")
    };
    let s = v / scale;
    let digits = if s.abs() >= 100.0 { 2 } else if s.abs() >= 10.0 { 3 } else { 4 };
    format!("{:.*} {}{}", digits, s, pre, unit)
}

/// Parse a value with an optional SI suffix and unit: "4.7k", "10 uF", "1meg", "2.2 mH".
pub fn parse_si(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut num_end = 0;
    for (i, ch) in s.char_indices() {
        if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' || ((ch == 'e' || ch == 'E') && i > 0 && s[i + 1..].starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+')) {
            num_end = i + ch.len_utf8();
        } else {
            break;
        }
    }
    let num: f64 = s[..num_end].parse().ok()?;
    let rest = s[num_end..].trim();
    let lower = rest.to_lowercase();
    let mult = if lower.starts_with("meg") {
        1e6
    } else if let Some(c) = rest.chars().next() {
        match c {
            'T' => 1e12,
            'G' => 1e9,
            'M' => 1e6,
            'k' | 'K' => 1e3,
            'm' => 1e-3,
            'u' | 'µ' | 'μ' => 1e-6,
            'n' => 1e-9,
            'p' => 1e-12,
            'f' => 1e-15,
            _ => 1.0,
        }
    } else {
        1.0
    };
    Some(num * mult)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn si() {
        assert_eq!(parse_si("4.7k"), Some(4700.0));
        assert!((parse_si("10 uF").unwrap() - 10e-6).abs() < 1e-15);
        assert_eq!(parse_si("1meg"), Some(1e6));
        assert_eq!(parse_si("2.2mH"), Some(2.2e-3));
        assert_eq!(parse_si("12 V"), Some(12.0));
        assert_eq!(parse_si("1e3"), Some(1000.0));
        assert_eq!(fmt_si(4700.0, "Ω"), "4.7 kΩ");
    }
}
