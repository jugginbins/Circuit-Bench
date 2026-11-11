//! The main window: state, editing commands, the interactive simulation loop and the frame update.

mod breadboard;
mod canvas;
mod panels;
mod props;
mod util;
mod windows;

use crate::draw::{self, Live, Pen};
use crate::theme::{self, pal, ThemeKind};
use crate::examples;
use crate::instruments::{self, Meter, Scope};
use crate::model::*;
use crate::sim::{self, Sim, Var};
use egui::{pos2, vec2, Align2, Color32, FontId, Id, Key, Pos2, Rect, RichText, Sense, Shape, Stroke, Ui};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;
use util::*;

const GRID: f32 = 10.0;
const FILE_EXT: &str = "cbs";

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Schematic,
    Breadboard,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BTool {
    Select,
    Jumper,
    Place(Kind),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BSel {
    Part(u64),
    Jumper(u64),
}

struct BDrag {
    start: (f32, f32),
    cur: (f32, f32),
    orig: Circuit,
    moved: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Sel {
    Comp(u64),
    Wire(u64),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    Wire,
    Place(Kind),
}

enum Drag {
    None,
    Move { start: GP, delta: GP, orig: Circuit },
    Band { start: Pos2 },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BottomTab {
    Results,
    Nets,
    Components,
    Erc,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AnalysisKind {
    Interactive,
    DcOp,
    Ac,
    Transient,
    DcSweep,
}

struct AnalysisCfg {
    sel: AnalysisKind,
    ac_fstart: f64,
    ac_fstop: f64,
    ac_ppd: usize,
    tr_stop: f64,
    tr_step: f64,
    tr_use_op: bool,
    dc_src: Option<u64>,
    dc_start: f64,
    dc_stop: f64,
    dc_incr: f64,
    out_vars: HashSet<String>,
    max_step: f64,
    ic_op: bool,
}

impl Default for AnalysisCfg {
    fn default() -> Self {
        AnalysisCfg {
            sel: AnalysisKind::Transient,
            ac_fstart: 1.0,
            ac_fstop: 10e6,
            ac_ppd: 20,
            tr_stop: 5e-3,
            tr_step: 5e-6,
            tr_use_op: false,
            dc_src: None,
            dc_start: 0.0,
            dc_stop: 10.0,
            dc_incr: 0.1,
            out_vars: HashSet::new(),
            max_step: 0.0,
            ic_op: false,
        }
    }
}

enum GraphTab {
    Op { title: String, rows: Vec<(String, f64, &'static str)> },
    Xy { title: String, xlabel: String, xunit: &'static str, series: Vec<sim::Series> },
    Ac { title: String, series: Vec<sim::AcSeries> },
}

impl GraphTab {
    fn title(&self) -> &str {
        match self {
            GraphTab::Op { title, .. } | GraphTab::Xy { title, .. } | GraphTab::Ac { title, .. } => title,
        }
    }
}

struct FileDlg {
    save: bool,
    netlist: bool,
    dir: PathBuf,
    name: String,
    err: Option<String>,
}

/// Preferences remembered between sessions.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Settings {
    theme: ThemeKind,
    show_grid: bool,
    show_flow: bool,
    smooth_zoom: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { theme: ThemeKind::Light, show_grid: true, show_flow: true, smooth_zoom: true }
    }
}

fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    home.join(".circuit_bench_settings.json")
}

/// Results shown by a Bode plotter panel.
#[derive(Default)]
struct BodeState {
    mag: Vec<[f64; 2]>,
    phase: Vec<[f64; 2]>,
    err: Option<String>,
    show_phase: bool,
    ppd: usize,
    computed_at: u64,
}

pub struct App {
    theme: ThemeKind,
    show_flow: bool,
    smooth_zoom: bool,
    zoom_target: f32,
    zoom_anchor: Option<(Pos2, egui::Vec2)>,
    pan_target: Option<egui::Vec2>,
    fit_snap: bool,
    flow_phase: HashMap<u64, f32>,
    held_button: Option<u64>,
    bodes: HashMap<u64, BodeState>,
    anim_clock: f32,
    last_update: Option<Instant>,
    view: View,
    sim_board: bool,
    bzoom: f32,
    bpan: egui::Vec2,
    bfit: bool,
    btool: BTool,
    bplace: Vec<BH>,
    bbody: Option<(f32, f32)>,
    bsel: Option<BSel>,
    bdrag: Option<BDrag>,
    jumper_start: Option<BH>,
    jcolor: usize,
    ctx_shift: bool,

    circuit: Circuit,
    undo: Vec<Circuit>,
    redo: Vec<Circuit>,
    file_path: Option<PathBuf>,
    dirty: bool,

    zoom: f32,
    pan: egui::Vec2,
    need_fit: bool,
    canvas_min: egui::Vec2,
    show_grid: bool,
    show_nets: bool,
    show_parts: bool,
    show_sheet: bool,

    tool: Tool,
    place_rot: u8,
    wiring: Option<Vec<GP>>,
    drag: Drag,
    selection: HashSet<Sel>,
    clipboard: (Vec<Component>, Vec<Wire>),
    paste_count: i32,
    hover_info: String,

    props: Option<u64>,
    props_orig: Option<Circuit>,
    wire_props: Option<u64>,
    open_inst: Vec<u64>,
    analysis_open: bool,
    cfg: AnalysisCfg,
    graphs: Vec<GraphTab>,
    graph_sel: usize,
    grapher_open: bool,
    file_dlg: Option<FileDlg>,
    about_open: bool,
    help_open: bool,
    confirm_new: Option<Option<usize>>,

    sim: Option<Sim>,
    running: bool,
    paused: bool,
    sim_error: Option<String>,
    meters: HashMap<u64, Meter>,
    scopes: HashMap<u64, Scope>,
    live: HashMap<u64, Live>,
    steps_last_frame: usize,
    h_used: f64,

    messages: Vec<String>,
    bottom_tab: BottomTab,
    last_frame: Option<Instant>,

    frame: u64,
    screenshot: Option<PathBuf>,
    shot_requested: bool,
}

fn default_dir() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."));
    let docs = home.join("Documents");
    if docs.is_dir() {
        docs
    } else {
        home
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, example: Option<usize>, open_file: Option<PathBuf>, theme_arg: Option<String>, view_arg: Option<String>) -> Self {
        let mut settings: Settings = std::fs::read_to_string(settings_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        if let Some(t) = theme_arg.and_then(|n| ThemeKind::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(&n))) {
            settings.theme = t;
        }
        theme::apply(&cc.egui_ctx, settings.theme);

        let ex = example.unwrap_or(0);
        let mut app = App {
            theme: settings.theme,
            show_flow: settings.show_flow,
            smooth_zoom: settings.smooth_zoom,
            zoom_target: 1.6,
            zoom_anchor: None,
            pan_target: None,
            fit_snap: true,
            flow_phase: HashMap::new(),
            held_button: None,
            bodes: HashMap::new(),
            anim_clock: 0.0,
            last_update: None,
            view: View::Schematic,
            sim_board: false,
            bzoom: 1.0,
            bpan: vec2(60.0, 60.0),
            bfit: true,
            btool: BTool::Select,
            bplace: Vec::new(),
            bbody: None,
            bsel: None,
            bdrag: None,
            jumper_start: None,
            jcolor: 0,
            ctx_shift: false,
            circuit: examples::by_index(ex),
            undo: Vec::new(),
            redo: Vec::new(),
            file_path: None,
            dirty: false,
            zoom: 1.6,
            pan: vec2(400.0, 300.0),
            need_fit: true,
            canvas_min: egui::Vec2::ZERO,
            show_grid: settings.show_grid,
            show_nets: false,
            show_parts: true,
            show_sheet: true,
            tool: Tool::Select,
            place_rot: 0,
            wiring: None,
            drag: Drag::None,
            selection: HashSet::new(),
            clipboard: (Vec::new(), Vec::new()),
            paste_count: 0,
            hover_info: String::new(),
            props: None,
            props_orig: None,
            wire_props: None,
            open_inst: Vec::new(),
            analysis_open: false,
            cfg: AnalysisCfg::default(),
            graphs: Vec::new(),
            graph_sel: 0,
            grapher_open: false,
            file_dlg: None,
            about_open: false,
            help_open: false,
            confirm_new: None,
            sim: None,
            running: false,
            paused: false,
            sim_error: None,
            meters: HashMap::new(),
            scopes: HashMap::new(),
            live: HashMap::new(),
            steps_last_frame: 0,
            h_used: 0.0,
            messages: vec![format!(
                "Welcome to Circuit Bench. The \"{}\" example is running — press the red Stop button to edit it, or open another one from File ▸ Examples.",
                examples::NAMES[ex.min(examples::NAMES.len() - 1)]
            )],
            bottom_tab: BottomTab::Results,
            last_frame: None,
            frame: 0,
            screenshot,
            shot_requested: false,
        };
        if let Some(p) = open_file {
            app.load_file(&p);
        }
        app.setup_example_instruments();
        // Open the first instrument panel and start the demo so the bench is alive on launch.
        if let Some(id) = app.circuit.components.iter().find(|c| c.kind == Kind::Oscilloscope).map(|c| c.id) {
            app.open_inst.push(id);
        } else if let Some(id) = app.circuit.components.iter().find(|c| c.kind == Kind::Multimeter).map(|c| c.id) {
            app.open_inst.push(id);
        }
        if view_arg.map(|v| v.eq_ignore_ascii_case("breadboard")).unwrap_or(false) {
            app.view = View::Breadboard;
            app.open_inst.clear();
            app.on_view_changed();
        }
        app.start_sim();
        app
    }

    fn setup_example_instruments(&mut self) {
        let srcs = || self.circuit.components.iter().filter(|c| matches!(c.kind, Kind::FuncGen | Kind::AcVoltage));
        let fmax = srcs().map(|c| c.params.freq).fold(0.0, f64::max);
        let amp = srcs().map(|c| c.params.value.abs() + c.params.offset.abs()).fold(0.0, f64::max);
        let ids: Vec<u64> = self.circuit.components.iter().filter(|c| c.kind == Kind::Oscilloscope).map(|c| c.id).collect();
        let has_opamp = self.circuit.components.iter().any(|c| c.kind == Kind::OpAmp);
        for id in ids {
            let sc = self.scopes.entry(id).or_default();
            if fmax > 0.0 {
                // about two periods on screen
                let mut tb = 1e-9;
                while tb * 10.0 < 2.0 / fmax {
                    tb = instruments::step_125(tb, true);
                }
                sc.timebase = tb;
            }
            if amp > 0.0 {
                let mut v = 0.01;
                while v * 4.0 < amp * 1.6 {
                    v = instruments::step_125(v, true);
                }
                sc.ch[0].scale = v;
                sc.ch[1].scale = v;
                if has_opamp {
                    sc.ch[1].scale = 2.0;
                }
            }
        }
    }

    // ------------------------------------------------------------ edit helpers

    fn push_undo(&mut self, orig: Circuit) {
        self.undo.push(orig);
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    fn checkpoint(&mut self) {
        let c = self.circuit.clone();
        self.push_undo(c);
    }

    /// Structural edits stop the simulation, like a real bench: power off before rewiring.
    fn edit_stop(&mut self) {
        if self.running {
            self.stop_sim();
            self.log("Simulation stopped because the circuit was edited.");
        }
    }

    fn do_undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.edit_stop();
            let cur = std::mem::replace(&mut self.circuit, prev);
            self.redo.push(cur);
            self.selection.clear();
            self.dirty = true;
        }
    }

    fn do_redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.edit_stop();
            let cur = std::mem::replace(&mut self.circuit, next);
            self.undo.push(cur);
            self.selection.clear();
            self.dirty = true;
        }
    }

    fn log(&mut self, s: impl Into<String>) {
        self.messages.push(s.into());
        if self.messages.len() > 500 {
            self.messages.remove(0);
        }
    }

    fn selected_comps(&self) -> HashSet<u64> {
        self.selection.iter().filter_map(|s| if let Sel::Comp(id) = s { Some(*id) } else { None }).collect()
    }

    fn selected_wires(&self) -> HashSet<u64> {
        self.selection.iter().filter_map(|s| if let Sel::Wire(id) = s { Some(*id) } else { None }).collect()
    }

    fn select_all(&mut self) {
        self.selection = self.circuit.components.iter().map(|c| Sel::Comp(c.id)).chain(self.circuit.wires.iter().map(|w| Sel::Wire(w.id))).collect();
    }

    fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.edit_stop();
        self.checkpoint();
        let comps = self.selected_comps();
        let wires = self.selected_wires();
        let mut dead_pins: HashSet<GP> = HashSet::new();
        let mut live_pins: HashSet<GP> = HashSet::new();
        for c in &self.circuit.components {
            if comps.contains(&c.id) {
                dead_pins.extend(c.pin_positions());
            } else {
                live_pins.extend(c.pin_positions());
            }
        }
        self.circuit.components.retain(|c| !comps.contains(&c.id));
        // wires that ended on a deleted part's pin go with it
        self.circuit.wires.retain(|w| {
            if wires.contains(&w.id) {
                return false;
            }
            let f = w.points[0];
            let l = *w.points.last().unwrap();
            let dead = |p: &GP| dead_pins.contains(p) && !live_pins.contains(p);
            !(dead(&f) || dead(&l))
        });
        self.open_inst.retain(|id| !comps.contains(id));
        self.selection.clear();
    }

    fn copy_selection(&mut self) {
        let comps = self.selected_comps();
        let wires = self.selected_wires();
        let cs: Vec<Component> = self.circuit.components.iter().filter(|c| comps.contains(&c.id)).cloned().collect();
        let ws: Vec<Wire> = self.circuit.wires.iter().filter(|w| wires.contains(&w.id)).cloned().collect();
        if !cs.is_empty() || !ws.is_empty() {
            self.clipboard = (cs, ws);
            self.paste_count = 0;
        }
    }

    fn paste(&mut self) {
        if self.clipboard.0.is_empty() && self.clipboard.1.is_empty() {
            return;
        }
        self.edit_stop();
        self.checkpoint();
        self.paste_count += 1;
        let off = GP::new(4 * self.paste_count, 4 * self.paste_count);
        self.selection.clear();
        let (cs, ws) = self.clipboard.clone();
        for mut c in cs {
            c.id = self.circuit.new_id();
            c.refdes = self.circuit.next_refdes(c.kind);
            c.pos = c.pos.add(off);
            self.selection.insert(Sel::Comp(c.id));
            self.circuit.components.push(c);
        }
        for mut w in ws {
            w.id = self.circuit.new_id();
            for p in w.points.iter_mut() {
                *p = p.add(off);
            }
            self.selection.insert(Sel::Wire(w.id));
            self.circuit.wires.push(w);
        }
    }

    fn rotate_selection(&mut self, ccw: bool) {
        let comps = self.selected_comps();
        if comps.is_empty() {
            return;
        }
        self.edit_stop();
        let orig = self.circuit.clone();
        for c in self.circuit.components.iter_mut() {
            if comps.contains(&c.id) {
                c.rot = (c.rot + if ccw { 3 } else { 1 }) % 4;
            }
        }
        reroute(&orig, &mut self.circuit, &comps, None, &HashSet::new());
        self.push_undo(orig);
    }

    fn new_circuit(&mut self, example: Option<usize>) {
        self.stop_sim();
        self.circuit = match example {
            Some(i) => examples::by_index(i),
            None => Circuit { title: "Untitled".into(), ..Default::default() },
        };
        self.undo.clear();
        self.redo.clear();
        self.selection.clear();
        self.file_path = None;
        self.dirty = false;
        self.open_inst.clear();
        self.scopes.clear();
        self.meters.clear();
        self.live.clear();
        self.need_fit = true;
        self.cfg.out_vars.clear();
        if let Some(i) = example {
            self.setup_example_instruments();
            self.log(format!("Opened example: {}", examples::NAMES[i.min(examples::NAMES.len() - 1)]));
            if let Some(id) = self.circuit.components.iter().find(|c| c.kind == Kind::Oscilloscope || c.kind == Kind::Multimeter).map(|c| c.id) {
                self.open_inst.push(id);
            }
        }
    }

    fn load_file(&mut self, p: &Path) {
        match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Circuit>(&s).map_err(|e| e.to_string())) {
            Ok(c) => {
                self.new_circuit(None);
                self.circuit = c;
                self.file_path = Some(p.to_path_buf());
                self.setup_example_instruments();
                self.log(format!("Opened {}", p.display()));
            }
            Err(e) => self.log(format!("Could not open {}: {}", p.display(), e)),
        }
    }

    fn save_file(&mut self, p: &Path) -> Result<(), String> {
        let txt = serde_json::to_string_pretty(&self.circuit).map_err(|e| e.to_string())?;
        std::fs::write(p, txt).map_err(|e| e.to_string())?;
        self.file_path = Some(p.to_path_buf());
        self.dirty = false;
        self.log(format!("Saved {}", p.display()));
        Ok(())
    }

    fn save(&mut self) {
        if let Some(p) = self.file_path.clone() {
            if let Err(e) = self.save_file(&p) {
                self.log(format!("Save failed: {}", e));
            }
        } else {
            self.open_file_dlg(true, false);
        }
    }

    fn open_file_dlg(&mut self, save: bool, netlist: bool) {
        let dir = self.file_path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(default_dir);
        let base = self.circuit.title.replace(['/', '\\', ':'], "_");
        let base = if base.trim().is_empty() { "circuit".to_string() } else { base };
        let name = if netlist {
            format!("{}.cir", base)
        } else if save {
            self.file_path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or(format!("{}.{}", base, FILE_EXT))
        } else {
            String::new()
        };
        self.file_dlg = Some(FileDlg { save, netlist, dir, name, err: None });
    }

    // ------------------------------------------------------------ simulation

    pub(super) fn set_theme(&mut self, ctx: &egui::Context, t: ThemeKind) {
        self.theme = t;
        theme::apply(ctx, t);
        self.save_settings();
    }

    fn save_settings(&self) {
        let st = Settings { theme: self.theme, show_grid: self.show_grid, show_flow: self.show_flow, smooth_zoom: self.smooth_zoom };
        if let Ok(txt) = serde_json::to_string_pretty(&st) {
            let _ = std::fs::write(settings_path(), txt);
        }
    }

    fn zoom_by(&mut self, f: f32) {
        self.zoom_target = (self.zoom_target * f).clamp(0.25, 8.0);
        self.zoom_anchor = None;
        self.pan_target = None;
    }

    /// Per-frame easing for glows, motors and buzzers so everything moves smoothly.
    fn animate(&mut self, dt: f32) {
        self.anim_clock += dt;
        let k = 1.0 - (-dt * 9.0).exp();
        let mut busy = false;
        for c in self.circuit.components.iter().chain(self.circuit.board.parts.iter().map(|p| &p.comp)) {
            let Some(l) = self.live.get_mut(&c.id) else {
                continue;
            };
            let target = if self.running { l.glow_target } else { 0.0 };
            l.glow += (target - l.glow) * k;
            if (target - l.glow).abs() < 0.002 {
                l.glow = target;
            } else {
                busy = true;
            }
            l.time += dt;
            if c.kind == Kind::Motor {
                l.spin = (l.spin + l.glow * 14.0 * dt) % std::f32::consts::TAU;
                busy |= l.glow > 0.0;
            }
            if c.kind == Kind::Buzzer && l.glow > 0.0 {
                busy = true;
            }
        }
        let _ = busy;
    }

    /// Push buttons are closed only while their key or the mouse is held.
    fn update_buttons(&mut self, ctx: &egui::Context) {
        let typing = ctx.wants_keyboard_input();
        let mouse_down = ctx.input(|i| i.pointer.primary_down());
        if !mouse_down {
            self.held_button = None;
        }
        let held = self.held_button;
        let running = self.running;
        for c in self.circuit.all_comps_mut() {
            if c.kind != Kind::PushButton {
                continue;
            }
            let key = if c.params.key == ' ' { Some(Key::Space) } else { Key::from_name(&c.params.key.to_ascii_uppercase().to_string()) };
            let by_key = !typing && key.map(|k| ctx.input(|i| i.key_down(k))).unwrap_or(false);
            let pressed = running && (by_key || held == Some(c.id));
            c.params.closed = pressed;
        }
    }

    fn start_sim(&mut self) {
        self.sim_error = None;
        for c in self.circuit.all_comps_mut() {
            if c.kind == Kind::Fuse {
                c.params.blown = false;
            }
        }
        self.sim_board = self.view == View::Breadboard;
        let made = if self.sim_board {
            let (bc, nl) = crate::board::sim_circuit(&self.circuit.board);
            if bc.components.is_empty() {
                Err("The breadboard is empty. Plug in some parts (or press Starter board) first.".to_string())
            } else if !nl.has_ground {
                Err("The breadboard needs a battery or signal source to power it.".to_string())
            } else {
                Sim::with_netlist(&bc, nl)
            }
        } else {
            Sim::new(&self.circuit)
        };
        match made {
            Ok(mut s) => {
                if self.cfg.ic_op {
                    if let Err(e) = s.dc_op() {
                        self.log(format!("Initial operating point failed, starting from zero: {}", e));
                    }
                }
                self.sim = Some(s);
                self.running = true;
                self.paused = false;
                for sc in self.scopes.values_mut() {
                    sc.reset();
                }
                self.meters.clear();
                for l in self.live.values_mut() {
                    l.reading = None;
                    l.glow_target = 0.0;
                }
                self.last_frame = None;
            }
            Err(e) => {
                self.sim_error = Some(e.clone());
                self.log(format!("Simulation error: {}", e));
                self.running = false;
                self.bottom_tab = BottomTab::Results;
            }
        }
    }

    fn stop_sim(&mut self) {
        self.sim = None;
        self.running = false;
        self.paused = false;
        for l in self.live.values_mut() {
            l.glow_target = 0.0;
        }
    }

    /// Switching between schematic and breadboard keeps a running simulation on the visible design.
    fn on_view_changed(&mut self) {
        self.hover_info.clear();
        if self.view == View::Breadboard && self.circuit.board.parts.is_empty() && self.circuit.board.jumpers.is_empty() {
            crate::board::starter_board(&mut self.circuit);
            self.bfit = true;
            self.log("Loaded the starter breadboard. Press Run to power it: hold B for the red LED, press Space for the buzzer, A / Shift+A turn the knob.");
        }
        if self.running && self.sim_board != (self.view == View::Breadboard) {
            self.stop_sim();
            self.start_sim();
        }
    }

    fn toggle_run(&mut self) {
        if self.running {
            self.stop_sim();
            self.log("Simulation stopped.");
        } else {
            self.start_sim();
            if self.running {
                self.log("Interactive simulation running.");
            }
        }
    }

    fn time_step(&self, sim: &Sim) -> f64 {
        if self.cfg.max_step > 0.0 {
            return self.cfg.max_step;
        }
        let f = sim.max_freq();
        if f > 0.0 {
            1.0 / (f * 200.0)
        } else {
            1e-5
        }
    }

    /// Advance the live simulation by about one frame of real time.
    fn run_sim_frame(&mut self) {
        let now = Instant::now();
        let dt_real = self.last_frame.map(|t| now.duration_since(t).as_secs_f64()).unwrap_or(1.0 / 60.0).clamp(0.001, 0.05);
        self.last_frame = Some(now);
        let Some(mut sim) = self.sim.take() else {
            return;
        };
        let simc = if self.sim_board {
            let (bc, nl) = crate::board::sim_circuit(&self.circuit.board);
            if !sim.refresh_with(&bc, nl) {
                self.start_sim();
                return;
            }
            bc
        } else {
            if !sim.refresh(&self.circuit) {
                // structure changed (e.g. meter mode): restart
                self.start_sim();
                return;
            }
            self.circuit.clone()
        };
        let h = self.time_step(&sim);
        self.h_used = h;
        let fmax = sim.max_freq();
        let window = if fmax > 0.0 { 3.0 / fmax } else { 0.0 };
        let target_steps = ((dt_real / h).ceil() as usize).max(1);
        let budget = std::time::Duration::from_millis(12);
        let mut steps = 0;
        let mut err = None;
        while steps < target_steps {
            if let Err(e) = sim.step(h) {
                err = Some(e);
                break;
            }
            steps += 1;
            sample(&simc, &sim, &mut self.meters, &mut self.scopes, window);
            if steps % 32 == 0 && now.elapsed() > budget {
                break;
            }
        }
        self.steps_last_frame = steps;
        if let Some(e) = err {
            self.log(format!("Simulation error at t = {}: {}", fmt_si(sim.t, "s"), e));
            self.sim_error = Some(e);
            self.running = false;
            return;
        }
        for sc in self.scopes.values_mut() {
            sc.update_display();
        }
        // fuses blow when their RMS current exceeds the rating
        let mut blown = Vec::new();
        for c in self.circuit.all_comps_mut() {
            if c.kind == Kind::Fuse && !c.params.blown {
                if let Some(m) = self.meters.get(&c.id) {
                    let rms = (m.mean().powi(2) + m.ac_rms().powi(2)).sqrt();
                    if rms > c.params.rating {
                        c.params.blown = true;
                        blown.push((c.refdes.clone(), rms, c.params.rating));
                    }
                }
            }
        }
        for (r, i, rating) in blown {
            self.log(format!("{} blew: {} through a {} fuse. Stop and run again to replace it.", r, fmt_si(i, "A"), fmt_si(rating, "A")));
        }
        self.update_live(fmax);
        self.sim = Some(sim);
    }

    fn update_live(&mut self, fmax: f64) {
        let comps: Vec<Component> = if self.sim_board { self.circuit.board.parts.iter().map(|p| p.comp.clone()).collect() } else { self.circuit.components.clone() };
        for c in &comps {
            let Some(m) = self.meters.get(&c.id) else {
                continue;
            };
            let live = self.live.entry(c.id).or_default();
            match c.kind {
                Kind::Voltmeter | Kind::Ammeter => {
                    let unit = if c.kind == Kind::Voltmeter { "V" } else { "A" };
                    let v = if c.params.ac { m.ac_rms() } else { m.mean() };
                    live.reading = Some(fmt_reading(v, unit));
                }
                Kind::Multimeter => live.reading = Some(multimeter_text(c, m)),
                Kind::Motor => {
                    let i = (m.mean().powi(2) + m.ac_rms().powi(2)).sqrt();
                    live.glow_target = if c.params.rating > 0.0 { (i / c.params.rating).clamp(0.0, 1.5) as f32 } else { 0.0 };
                    live.reading = Some(fmt_reading(i, "A"));
                }
                Kind::Buzzer => {
                    let i = (m.mean().powi(2) + m.ac_rms().powi(2)).sqrt();
                    live.glow_target = if c.params.rating > 0.0 && i > 0.1 * c.params.rating { (i / c.params.rating).clamp(0.3, 1.0) as f32 } else { 0.0 };
                }
                Kind::Wattmeter => live.reading = Some(fmt_reading(m.mean(), "W")),
                Kind::FreqCounter => {
                    let st = instruments::freq_stats(m, c.params.value);
                    live.reading = Some(match st {
                        Some(s) if s.freq > 0.0 => fmt_si(s.freq, "Hz"),
                        _ => "--- Hz".into(),
                    });
                }
                Kind::Probe => {
                    live.reading = Some(if fmax > 0.0 {
                        format!("Vpp {}  Vrms {}", fmt_si(m.peak_to_peak(), "V"), fmt_si((m.ac_rms().powi(2) + m.mean().powi(2)).sqrt(), "V"))
                    } else {
                        format!("V: {}", fmt_reading(m.mean(), "V"))
                    });
                }
                Kind::Led => {
                    let i = m.mean_recent(0.05);
                    live.glow_target = if i > 0.5e-3 { (i / 0.01).clamp(0.15, 1.0) as f32 } else { 0.0 };
                    live.reading = Some(fmt_reading(i, "A"));
                }
                Kind::Lamp => {
                    let p = m.mean_recent(0.05);
                    live.glow_target = if c.params.power > 0.0 { (p / c.params.power).clamp(0.0, 1.0) as f32 } else { 0.0 };
                    live.reading = Some(fmt_reading(p, "W"));
                }
                _ => {}
            }
        }
    }

    // ------------------------------------------------------------ keyboard

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            let egui::Event::Key { key, pressed: true, modifiers, repeat: false, .. } = ev else {
                continue;
            };
            let cmd = modifiers.command;
            // interactive keys for switches and potentiometers while simulating
            if self.running && !cmd && self.interactive_key(key, modifiers.shift) {
                continue;
            }
            match key {
                Key::Z if cmd && modifiers.shift => self.do_redo(),
                Key::Z if cmd => self.do_undo(),
                Key::Y if cmd => self.do_redo(),
                Key::C if cmd => self.copy_selection(),
                Key::X if cmd => {
                    self.copy_selection();
                    self.delete_selection();
                }
                Key::V if cmd => self.paste(),
                Key::A if cmd => self.select_all(),
                Key::S if cmd => self.save(),
                Key::O if cmd => self.open_file_dlg(false, false),
                Key::N if cmd => self.confirm_new = Some(None),
                Key::R if cmd => self.rotate_selection(modifiers.shift),
                Key::R => {
                    if let Tool::Place(_) = self.tool {
                        self.place_rot = (self.place_rot + 1) % 4;
                    } else {
                        self.rotate_selection(modifiers.shift);
                    }
                }
                Key::Delete | Key::Backspace if self.view == View::Breadboard => self.board_delete(),
                Key::Delete | Key::Backspace => self.delete_selection(),
                Key::Escape if self.view == View::Breadboard => {
                    self.bplace.clear();
                    self.bbody = None;
                    self.jumper_start = None;
                    self.btool = BTool::Select;
                }
                Key::Escape => {
                    if self.wiring.is_some() {
                        self.wiring = None;
                    } else {
                        self.tool = Tool::Select;
                        self.selection.clear();
                    }
                }
                Key::W if !cmd => self.tool = if self.tool == Tool::Wire { Tool::Select } else { Tool::Wire },
                Key::F5 => self.toggle_run(),
                Key::F6 => {
                    if self.running {
                        self.paused = !self.paused;
                    }
                }
                Key::F7 => self.need_fit = true,
                Key::Plus | Key::Equals if cmd => self.zoom_by(1.25),
                Key::Minus if cmd => self.zoom_by(1.0 / 1.25),
                _ => {}
            }
        }
    }

    fn interactive_key(&mut self, key: Key, shift: bool) -> bool {
        let ch = match key {
            Key::Space => ' ',
            k => {
                let n = k.name();
                if n.chars().count() != 1 {
                    return false;
                }
                n.chars().next().unwrap()
            }
        };
        let mut used = false;
        for c in self.circuit.all_comps_mut() {
            if c.params.key.to_ascii_uppercase() != ch.to_ascii_uppercase() {
                continue;
            }
            match c.kind {
                Kind::Switch | Kind::SpdtSwitch => {
                    c.params.closed = !c.params.closed;
                    used = true;
                }
                Kind::PushButton => used = true,
                Kind::Potentiometer => {
                    let d = if shift { -c.params.increment } else { c.params.increment };
                    c.params.setting = (c.params.setting + d).clamp(0.0, 100.0);
                    used = true;
                }
                _ => {}
            }
        }
        used
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame += 1;
        let now = Instant::now();
        let dt = self.last_update.map(|t| now.duration_since(t).as_secs_f32()).unwrap_or(1.0 / 60.0).clamp(0.001, 0.1);
        self.last_update = Some(now);
        self.handle_keys(ctx);
        self.update_buttons(ctx);
        if self.running && !self.paused {
            self.run_sim_frame();
        } else {
            self.last_frame = None;
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.menu_bar(ui, ctx));
        egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(8.0, 5.0)))
            .show(ctx, |ui| self.toolbar(ui));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status_bar(ui));
        if self.show_sheet {
            egui::TopBottomPanel::bottom("sheet").resizable(true).default_height(150.0).min_height(80.0).show(ctx, |ui| self.spreadsheet(ui));
        }
        if self.show_parts {
            egui::SidePanel::left("parts").resizable(true).default_width(215.0).show(ctx, |ui| {
                if self.view == View::Breadboard {
                    self.board_parts_panel(ui)
                } else {
                    self.parts_panel(ui)
                }
            });
        }
        egui::SidePanel::right("instruments").resizable(true).default_width(200.0).show(ctx, |ui| self.right_panel(ui));
        self.ctx_shift = ctx.input(|i| i.modifiers.shift);
        egui::CentralPanel::default().frame(egui::Frame::none().fill(pal().sheet)).show(ctx, |ui| {
            egui::Frame::none().fill(pal().panel).inner_margin(egui::Margin::symmetric(8.0, 5.0)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let before = self.view;
                    for (v, name) in [(View::Schematic, "Schematic"), (View::Breadboard, "Breadboard")] {
                        let on = self.view == v;
                        let b = egui::Button::new(RichText::new(name).strong().color(if on { Color32::WHITE } else { pal().text })).fill(if on { pal().accent } else { pal().widget }).rounding(12.0).min_size(vec2(104.0, 24.0));
                        if ui.add(b).clicked() {
                            self.view = v;
                        }
                    }
                    if self.view != before {
                        self.on_view_changed();
                    }
                    ui.separator();
                    if self.view == View::Breadboard {
                        self.board_tools(ui);
                    } else {
                        ui.label(RichText::new("Draw the circuit diagram here, or switch to Breadboard to build it with real parts.").small().weak());
                    }
                });
            });
            if self.view == View::Breadboard {
                self.board_view(ui)
            } else {
                self.canvas(ui)
            }
        });
        self.animate(dt);

        self.instrument_windows(ctx);
        self.props_window(ctx);
        self.wire_window(ctx);
        self.analysis_window(ctx);
        self.grapher_window(ctx);
        self.file_window(ctx);
        self.misc_windows(ctx);

        if self.frame % 30 == 1 {
            let name = self
                .file_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| if self.circuit.title.is_empty() { "Untitled".into() } else { self.circuit.title.clone() });
            let title = format!("Circuit Bench — {}{}", name, if self.dirty { " *" } else { "" });
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }

        // --screenshot: draw ~120 frames, grab the window, save and quit
        if let Some(path) = self.screenshot.clone() {
            if self.frame >= 120 && !self.shot_requested {
                self.shot_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
            let shot = ctx.input(|i| i.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }));
            if let Some(img) = shot {
                if let Err(e) = save_png(&path, &img) {
                    eprintln!("screenshot failed: {}", e);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if self.frame > 900 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
        let animating = self.live.values().any(|l| (l.glow - if self.running { l.glow_target } else { 0.0 }).abs() > 0.002 || l.glow > 0.0)
            || (self.zoom_target - self.zoom).abs() > 1e-4
            || self.pan_target.is_some();
        if (self.running && !self.paused) || animating {
            ctx.request_repaint();
        }
    }
}
