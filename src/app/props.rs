//! Editable settings for every kind of part (shown in the Properties window).

use super::util::key_field;
use super::*;

const DIODE_PRESETS: [(&str, f64, f64, f64); 5] = [
    ("Generic silicon", 2.52e-9, 1.752, 0.0),
    ("Small-signal", 4.35e-9, 1.906, 100.0),
    ("Rectifier", 14.1e-9, 1.984, 1000.0),
    ("Schottky", 3.0e-6, 1.04, 40.0),
    ("Germanium", 1.0e-6, 1.3, 50.0),
];

fn row_si(ui: &mut Ui, pid: Id, label: &str, k: u32, v: &mut f64, unit: &str) {
    ui.label(label);
    instruments::si_field(ui, pid.with(k), v, unit, 150.0);
    ui.end_row();
}

fn row_drag(ui: &mut Ui, label: &str, v: &mut f64, suffix: &str, range: std::ops::RangeInclusive<f64>, speed: f64) {
    ui.label(label);
    ui.add(egui::DragValue::new(v).suffix(suffix).clamp_range(range).speed(speed));
    ui.end_row();
}

fn row_key(ui: &mut Ui, key: &mut char) {
    ui.label("Key");
    key_field(ui, key);
    ui.end_row();
}

fn row_waveform(ui: &mut Ui, p: &mut Params, pid: Id, amp_unit: &str) {
    row_si(ui, pid, "Amplitude (peak)", 1, &mut p.value, amp_unit);
    row_si(ui, pid, "Frequency", 2, &mut p.freq, "Hz");
    row_drag(ui, "Phase", &mut p.phase, "°", -360.0..=360.0, 1.0);
    row_si(ui, pid, "DC offset", 3, &mut p.offset, amp_unit);
}

pub(super) fn part_fields(ui: &mut Ui, kind: Kind, p: &mut Params, pid: Id) {
    match kind {
        Kind::Resistor => row_si(ui, pid, "Resistance", 1, &mut p.value, "Ω"),
        Kind::Capacitor => {
            row_si(ui, pid, "Capacitance", 1, &mut p.value, "F");
            row_si(ui, pid, "Initial voltage", 2, &mut p.ic, "V");
        }
        Kind::Inductor => {
            row_si(ui, pid, "Inductance", 1, &mut p.value, "H");
            row_si(ui, pid, "Initial current", 2, &mut p.ic, "A");
        }
        Kind::Potentiometer => {
            row_si(ui, pid, "Resistance", 1, &mut p.value, "Ω");
            ui.label("Setting");
            ui.add(egui::Slider::new(&mut p.setting, 0.0..=100.0).suffix(" %"));
            ui.end_row();
            row_drag(ui, "Increment", &mut p.increment, " %", 0.1..=50.0, 0.5);
            row_key(ui, &mut p.key);
        }
        Kind::DcVoltage => row_si(ui, pid, "Voltage", 1, &mut p.value, "V"),
        Kind::DcCurrent => row_si(ui, pid, "Current", 1, &mut p.value, "A"),
        Kind::AcVoltage => row_waveform(ui, p, pid, "V"),
        Kind::AcCurrent => row_waveform(ui, p, pid, "A"),
        Kind::ClockSource => {
            row_si(ui, pid, "High level", 1, &mut p.value, "V");
            row_si(ui, pid, "Low level", 2, &mut p.offset, "V");
            row_si(ui, pid, "Frequency", 3, &mut p.freq, "Hz");
            ui.label("Duty cycle");
            ui.add(egui::Slider::new(&mut p.duty, 1.0..=99.0).suffix(" %"));
            ui.end_row();
        }
        Kind::Ground => {}
        Kind::Diode => {
            ui.label("Model");
            egui::ComboBox::from_id_source(pid.with("dm")).selected_text("Choose a preset…").show_ui(ui, |ui| {
                for (n, is, nn, bv) in DIODE_PRESETS {
                    if ui.selectable_label(false, n).clicked() {
                        p.is = is;
                        p.n = nn;
                        p.bv = bv;
                    }
                }
            });
            ui.end_row();
            row_si(ui, pid, "Saturation current (Is)", 1, &mut p.is, "A");
            row_drag(ui, "Emission coefficient (N)", &mut p.n, "", 0.5..=4.0, 0.01);
            row_si(ui, pid, "Breakdown voltage (0 = none)", 2, &mut p.bv, "V");
        }
        Kind::Zener => {
            row_si(ui, pid, "Zener voltage", 1, &mut p.value, "V");
            row_si(ui, pid, "Saturation current (Is)", 2, &mut p.is, "A");
            row_drag(ui, "Emission coefficient (N)", &mut p.n, "", 0.5..=4.0, 0.01);
        }
        Kind::Led => {
            ui.label("Colour");
            ui.horizontal_wrapped(|ui| {
                for c in LedColor::ALL {
                    let [r, g, b] = c.rgb();
                    let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                    ui.painter().circle_filled(rect.center(), 8.0, Color32::from_rgb(r, g, b));
                    if p.led == c {
                        ui.painter().circle_stroke(rect.center(), 9.5, Stroke::new(2.0, pal().text));
                    }
                    if resp.on_hover_text(format!("{} (about {} V)", c.name(), c.forward_voltage())).clicked() {
                        p.led = c;
                    }
                }
            });
            ui.end_row();
            row_si(ui, pid, "Forward voltage (0 = by colour)", 1, &mut p.vf, "V");
            row_si(ui, pid, "Full-brightness current", 2, &mut p.rating, "A");
        }
        Kind::Switch => {
            row_key(ui, &mut p.key);
            ui.label("State");
            ui.checkbox(&mut p.closed, "Closed");
            ui.end_row();
            row_si(ui, pid, "On resistance", 1, &mut p.r_internal, "Ω");
        }
        Kind::PushButton => {
            row_key(ui, &mut p.key);
            ui.label("Type");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut p.normally_closed, false, "Normally open");
                ui.selectable_value(&mut p.normally_closed, true, "Normally closed");
            });
            ui.end_row();
            row_si(ui, pid, "On resistance", 1, &mut p.r_internal, "Ω");
        }
        Kind::SpdtSwitch => {
            row_key(ui, &mut p.key);
            ui.label("Position");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut p.closed, false, "C → A");
                ui.selectable_value(&mut p.closed, true, "C → B");
            });
            ui.end_row();
            row_si(ui, pid, "On resistance", 1, &mut p.r_internal, "Ω");
        }
        Kind::Fuse => {
            row_si(ui, pid, "Current rating", 1, &mut p.rating, "A");
            ui.label("State");
            ui.horizontal(|ui| {
                ui.label(if p.blown { "Blown" } else { "Intact" });
                if p.blown && ui.button("Replace").clicked() {
                    p.blown = false;
                }
            });
            ui.end_row();
        }
        Kind::Lamp => {
            row_si(ui, pid, "Rated voltage", 1, &mut p.value, "V");
            row_si(ui, pid, "Rated power", 2, &mut p.power, "W");
        }
        Kind::Motor => {
            row_si(ui, pid, "Winding resistance", 1, &mut p.value, "Ω");
            row_si(ui, pid, "Rated current", 2, &mut p.rating, "A");
        }
        Kind::Buzzer => {
            row_si(ui, pid, "Resistance", 1, &mut p.value, "Ω");
            row_si(ui, pid, "Rated current", 2, &mut p.rating, "A");
            row_si(ui, pid, "Tone", 3, &mut p.freq, "Hz");
        }
        Kind::OpAmp => {
            row_si(ui, pid, "Open-loop gain", 1, &mut p.value, "");
            row_si(ui, pid, "Output swing (±)", 2, &mut p.power, "V");
        }
        Kind::Voltmeter | Kind::Ammeter => {
            ui.label("Mode");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut p.ac, false, "DC");
                ui.selectable_value(&mut p.ac, true, "AC (RMS)");
            });
            ui.end_row();
            let label = if kind == Kind::Voltmeter { "Internal resistance" } else { "Internal resistance (0 = ideal)" };
            row_si(ui, pid, label, 1, &mut p.r_internal, "Ω");
        }
        Kind::Probe => {}
        Kind::Multimeter => {
            ui.label("Measure");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut p.meter, MeterMode::Volts, "V");
                ui.selectable_value(&mut p.meter, MeterMode::Amps, "A");
                ui.selectable_value(&mut p.meter, MeterMode::Ohms, "Ω");
                ui.selectable_value(&mut p.meter, MeterMode::Decibel, "dB");
                ui.separator();
                ui.selectable_value(&mut p.ac, false, "DC");
                ui.selectable_value(&mut p.ac, true, "AC");
            });
            ui.end_row();
            row_si(ui, pid, "Voltmeter resistance", 1, &mut p.r_internal, "Ω");
            row_si(ui, pid, "Ammeter resistance (0 = ideal)", 2, &mut p.r_amm, "Ω");
            row_si(ui, pid, "Ohmmeter test current", 3, &mut p.i_test, "A");
        }
        Kind::FuncGen => {
            row_si(ui, pid, "Amplitude (peak)", 1, &mut p.value, "V");
            row_si(ui, pid, "Frequency", 2, &mut p.freq, "Hz");
            row_si(ui, pid, "Offset", 3, &mut p.offset, "V");
        }
        Kind::BodePlotter => {
            row_si(ui, pid, "Start frequency", 1, &mut p.freq, "Hz");
            row_si(ui, pid, "Stop frequency", 2, &mut p.f_stop, "Hz");
        }
        Kind::FreqCounter => row_si(ui, pid, "Trigger level", 1, &mut p.value, "V"),
        Kind::Oscilloscope | Kind::Wattmeter => {}
    }
    if kind != Kind::Ground {
        ui.label("Show value on sheet");
        ui.checkbox(&mut p.show_value, "");
        ui.end_row();
    }
}
