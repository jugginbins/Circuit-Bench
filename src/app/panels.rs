//! Menu bar, toolbar, parts bin, instrument bar, spreadsheet view and status bar.

use super::util::*;
use super::*;

impl App {
    pub(super) fn menu_bar(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New                    Ctrl+N").clicked() {
                    self.confirm_new = Some(None);
                    ui.close_menu();
                }
                if ui.button("Open…                  Ctrl+O").clicked() {
                    self.open_file_dlg(false, false);
                    ui.close_menu();
                }
                ui.menu_button("Examples", |ui| {
                    for (i, n) in examples::NAMES.iter().enumerate() {
                        if ui.button(*n).clicked() {
                            self.confirm_new = Some(Some(i));
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                if ui.button("Save                   Ctrl+S").clicked() {
                    self.save();
                    ui.close_menu();
                }
                if ui.button("Save as…").clicked() {
                    self.open_file_dlg(true, false);
                    ui.close_menu();
                }
                if ui.button("Export SPICE netlist…").clicked() {
                    self.open_file_dlg(true, true);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo              Ctrl+Z")).clicked() {
                    self.do_undo();
                    ui.close_menu();
                }
                if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo              Ctrl+Y")).clicked() {
                    self.do_redo();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Cut                 Ctrl+X").clicked() {
                    self.copy_selection();
                    self.delete_selection();
                    ui.close_menu();
                }
                if ui.button("Copy                Ctrl+C").clicked() {
                    self.copy_selection();
                    ui.close_menu();
                }
                if ui.button("Paste               Ctrl+V").clicked() {
                    self.paste();
                    ui.close_menu();
                }
                if ui.button("Delete              Del").clicked() {
                    self.delete_selection();
                    ui.close_menu();
                }
                if ui.button("Select all          Ctrl+A").clicked() {
                    self.select_all();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Rotate 90° clockwise          Ctrl+R").clicked() {
                    self.rotate_selection(false);
                    ui.close_menu();
                }
                if ui.button("Rotate 90° counter-clockwise").clicked() {
                    self.rotate_selection(true);
                    ui.close_menu();
                }
                if ui.button("Properties…").clicked() {
                    if let Some(id) = self.selected_comps().into_iter().next() {
                        self.open_props(id);
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Circuit title…").clicked() {
                    self.props = Some(0);
                    self.props_orig = Some(self.circuit.clone());
                    ui.close_menu();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Zoom in        Ctrl++").clicked() {
                    self.zoom_by(1.25);
                }
                if ui.button("Zoom out       Ctrl+−").clicked() {
                    self.zoom_by(1.0 / 1.25);
                }
                if ui.button("Zoom to fit    F7").clicked() {
                    self.need_fit = true;
                }
                ui.separator();
                ui.menu_button("Theme", |ui| {
                    for t in ThemeKind::ALL {
                        if theme_choice(ui, t, self.theme == t).clicked() {
                            self.set_theme(ctx, t);
                            ui.close_menu();
                        }
                    }
                });
                let dark = self.theme.palette().dark;
                if ui.button(if dark { "Switch to light mode" } else { "Switch to dark mode" }).clicked() {
                    self.set_theme(ctx, if dark { ThemeKind::Light } else { ThemeKind::Dark });
                    ui.close_menu();
                }
                ui.separator();
                let mut changed = false;
                changed |= ui.checkbox(&mut self.show_grid, "Show grid").changed();
                changed |= ui.checkbox(&mut self.show_flow, "Animate current flow").changed();
                changed |= ui.checkbox(&mut self.smooth_zoom, "Smooth zoom").changed();
                if changed {
                    self.save_settings();
                }
                ui.checkbox(&mut self.show_nets, "Show net numbers (while simulating)");
                ui.checkbox(&mut self.show_parts, "Parts bin");
                ui.checkbox(&mut self.show_sheet, "Spreadsheet view");
                if ui.button("Grapher").clicked() {
                    self.grapher_open = true;
                    ui.close_menu();
                }
            });
            ui.menu_button("Place", |ui| {
                for (group, kinds) in PART_GROUPS {
                    ui.menu_button(*group, |ui| {
                        for k in kinds.iter() {
                            if ui.button(k.name()).clicked() {
                                self.tool = Tool::Place(*k);
                                self.place_rot = 0;
                                ui.close_menu();
                            }
                        }
                    });
                }
                ui.menu_button("Instruments", |ui| {
                    for k in INSTRUMENTS {
                        if ui.button(k.name()).clicked() {
                            self.tool = Tool::Place(k);
                            self.place_rot = 0;
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                if ui.button("Wire                W").clicked() {
                    self.tool = Tool::Wire;
                    ui.close_menu();
                }
            });
            ui.menu_button("Simulate", |ui| {
                if ui.button(if self.running { "Stop                F5" } else { "Run                 F5" }).clicked() {
                    self.toggle_run();
                    ui.close_menu();
                }
                if ui.add_enabled(self.running, egui::Button::new(if self.paused { "Resume              F6" } else { "Pause               F6" })).clicked() {
                    self.paused = !self.paused;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Analyses and simulation…").clicked() {
                    self.analysis_open = true;
                    ui.close_menu();
                }
                for (k, n) in [(AnalysisKind::DcOp, "DC operating point"), (AnalysisKind::Ac, "AC sweep"), (AnalysisKind::Transient, "Transient"), (AnalysisKind::DcSweep, "DC sweep")] {
                    if ui.button(format!("    {}", n)).clicked() {
                        self.cfg.sel = k;
                        self.analysis_open = true;
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("Interactive simulation settings…").clicked() {
                    self.cfg.sel = AnalysisKind::Interactive;
                    self.analysis_open = true;
                    ui.close_menu();
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Electrical rules check").clicked() {
                    self.bottom_tab = BottomTab::Erc;
                    self.show_sheet = true;
                    ui.close_menu();
                }
                if ui.button("Parts list").clicked() {
                    self.bottom_tab = BottomTab::Components;
                    self.show_sheet = true;
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Quick guide").clicked() {
                    self.help_open = true;
                    ui.close_menu();
                }
                if ui.button("About Circuit Bench").clicked() {
                    self.about_open = true;
                    ui.close_menu();
                }
            });
        });
    }

    pub(super) fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().button_padding = vec2(8.0, 4.0);
            if ui.button("New").on_hover_text("New circuit (Ctrl+N)").clicked() {
                self.confirm_new = Some(None);
            }
            if ui.button("Open").on_hover_text("Open a saved circuit (Ctrl+O)").clicked() {
                self.open_file_dlg(false, false);
            }
            if ui.button("Save").on_hover_text("Save (Ctrl+S)").clicked() {
                self.save();
            }
            ui.separator();
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo")).clicked() {
                self.do_undo();
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo")).clicked() {
                self.do_redo();
            }
            ui.separator();
            let has_sel = !self.selection.is_empty();
            if ui.add_enabled(has_sel, egui::Button::new("Copy")).clicked() {
                self.copy_selection();
            }
            if ui.add_enabled(!self.clipboard.0.is_empty() || !self.clipboard.1.is_empty(), egui::Button::new("Paste")).clicked() {
                self.paste();
            }
            if ui.add_enabled(has_sel, egui::Button::new("Rotate")).on_hover_text("Rotate 90° (Ctrl+R)").clicked() {
                self.rotate_selection(false);
            }
            if ui.add_enabled(has_sel, egui::Button::new("Delete")).clicked() {
                self.delete_selection();
            }
            ui.separator();
            if ui.button("Zoom +").clicked() {
                self.zoom_by(1.25);
            }
            if ui.button("Zoom −").clicked() {
                self.zoom_by(1.0 / 1.25);
            }
            if ui.button("Fit").on_hover_text("Zoom to fit (F7)").clicked() {
                self.need_fit = true;
            }
            ui.separator();
            if ui.add(egui::Button::new("Select").selected(self.tool == Tool::Select)).clicked() {
                self.tool = Tool::Select;
                self.wiring = None;
            }
            if ui.add(egui::Button::new("Wire").selected(self.tool == Tool::Wire)).on_hover_text("Draw wires (W). You can also click any pin to start a wire.").clicked() {
                self.tool = if self.tool == Tool::Wire { Tool::Select } else { Tool::Wire };
            }
            ui.separator();
            if ui.button("Analyses…").clicked() {
                self.analysis_open = true;
            }
            if ui.button("Grapher").clicked() {
                self.grapher_open = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if sim_button(ui, SimIcon::Stop, false, "Stop simulation").clicked() && self.running {
                    self.stop_sim();
                    self.log("Simulation stopped.");
                }
                if sim_button(ui, SimIcon::Pause, self.running && self.paused, "Pause / resume (F6)").clicked() && self.running {
                    self.paused = !self.paused;
                }
                if sim_button(ui, SimIcon::Run, self.running && !self.paused, "Run interactive simulation (F5)").clicked() {
                    if !self.running {
                        self.start_sim();
                        if self.running {
                            self.log("Interactive simulation running.");
                        }
                    } else {
                        self.paused = false;
                    }
                }
                ui.label(RichText::new("Simulate").strong());
                ui.separator();
                let dark = self.theme.palette().dark;
                if mode_toggle(ui, dark).on_hover_text(if dark { "Switch to light mode" } else { "Switch to dark mode" }).clicked() {
                    self.set_theme(ui.ctx(), if dark { ThemeKind::Light } else { ThemeKind::Dark });
                }
                let mut pick = self.theme;
                egui::ComboBox::from_id_source("theme_pick").width(110.0).selected_text(format!("Theme: {}", self.theme.name())).show_ui(ui, |ui| {
                    for t in ThemeKind::ALL {
                        if theme_choice(ui, t, self.theme == t).clicked() {
                            pick = t;
                        }
                    }
                });
                if pick != self.theme {
                    self.set_theme(ui.ctx(), pick);
                }
            });
        });
    }

    pub(super) fn parts_panel(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.heading("Parts bin");
        ui.label(RichText::new("Click a part, then click the sheet to place it.").small().weak());
        ui.add_space(2.0);
        egui::ScrollArea::vertical().id_source("parts_scroll").max_height(ui.available_height() * 0.66).show(ui, |ui| {
            for (group, kinds) in PART_GROUPS {
                egui::CollapsingHeader::new(RichText::new(*group).strong()).default_open(true).show(ui, |ui| {
                    for k in kinds.iter() {
                        if part_row(ui, *k, self.tool == Tool::Place(*k)).clicked() {
                            self.tool = Tool::Place(*k);
                            self.place_rot = 0;
                            self.wiring = None;
                        }
                    }
                });
            }
        });
        ui.separator();
        ui.label(RichText::new("Design toolbox").strong());
        egui::ScrollArea::vertical().id_source("toolbox_scroll").show(ui, |ui| {
            ui.label(format!("{}", if self.circuit.title.is_empty() { "Untitled" } else { &self.circuit.title }));
            let mut items: Vec<(String, u64, Kind)> = self.circuit.components.iter().filter(|c| c.kind != Kind::Ground).map(|c| (c.refdes.clone(), c.id, c.kind)).collect();
            items.sort_by(|a, b| natural_key(&a.0).cmp(&natural_key(&b.0)));
            for (name, id, kind) in items {
                let sel = self.selection.contains(&Sel::Comp(id));
                let r = ui.selectable_label(sel, format!("    {}  —  {}", name, kind.name()));
                if r.clicked() {
                    self.selection.clear();
                    self.selection.insert(Sel::Comp(id));
                }
                if r.double_clicked() {
                    self.open_props(id);
                }
            }
        });
    }

    pub(super) fn right_panel(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.heading("Instruments");
        ui.label(RichText::new("Place, wire up, then double-click to open its panel.").small().weak());
        for k in INSTRUMENTS {
            if part_row(ui, k, self.tool == Tool::Place(k)).clicked() {
                self.tool = Tool::Place(k);
                self.place_rot = 0;
            }
        }
        let insts: Vec<(u64, String)> = self.circuit.components.iter().filter(|c| c.kind.is_instrument()).map(|c| (c.id, c.refdes.clone())).collect();
        if !insts.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Open panel").small());
            ui.horizontal_wrapped(|ui| {
                for (id, name) in insts {
                    if ui.small_button(name).clicked() && !self.open_inst.contains(&id) {
                        self.open_inst.push(id);
                    }
                }
            });
        }
        ui.separator();
        ui.heading("Selection");
        let comps = self.selected_comps();
        if comps.len() == 1 {
            let id = *comps.iter().next().unwrap();
            if let Some(c) = self.circuit.comp(id).cloned() {
                ui.label(RichText::new(format!("{}  ({})", c.refdes, c.kind.name())).strong());
                let v = c.value_label();
                if !v.is_empty() {
                    ui.label(v);
                }
                if let Some(l) = self.live.get(&id).and_then(|l| l.reading.clone()) {
                    ui.label(RichText::new(l).monospace().color(Color32::from_rgb(0, 110, 40)));
                }
                if let Some(sim) = self.sim.as_ref().filter(|_| !self.sim_board) {
                    if let Some(ci) = self.circuit.components.iter().position(|x| x.id == id) {
                        let names: Vec<&str> = c.kind.pins().iter().map(|p| p.2).collect();
                        for (k, n) in sim.nl.comp_nets[ci].iter().enumerate() {
                            ui.label(RichText::new(format!("pin {} → net {}  {}", names[k], n, fmt_reading(sim.v(*n), "V"))).small().monospace());
                        }
                        if !matches!(c.kind, Kind::Ground | Kind::Probe | Kind::Oscilloscope | Kind::FuncGen | Kind::Multimeter | Kind::Potentiometer) {
                            ui.label(RichText::new(format!("I = {}", fmt_reading(sim.comp_current(ci), "A"))).small().monospace());
                        }
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Properties…").clicked() {
                        self.open_props(id);
                    }
                    if ui.button("Rotate").clicked() {
                        self.rotate_selection(false);
                    }
                });
            }
        } else if self.selection.is_empty() {
            ui.label(RichText::new("Nothing selected").weak());
        } else {
            ui.label(format!("{} items selected", self.selection.len()));
        }
        if !self.hover_info.is_empty() {
            ui.separator();
            ui.label(RichText::new(&self.hover_info).monospace().color(Color32::from_rgb(0, 90, 160)));
        }
    }

    pub(super) fn spreadsheet(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Results, "Results");
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Nets, "Nets");
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Components, "Components");
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Erc, "Electrical rules check");
            if self.bottom_tab == BottomTab::Results {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Clear").clicked() {
                        self.messages.clear();
                    }
                });
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(self.bottom_tab == BottomTab::Results).show(ui, |ui| match self.bottom_tab {
            BottomTab::Results => {
                for m in &self.messages {
                    ui.label(RichText::new(m).monospace().size(12.0));
                }
            }
            BottomTab::Nets => {
                let nl = sim::build_netlist(&self.circuit);
                let mut pins: Vec<Vec<String>> = vec![Vec::new(); nl.n_nets];
                for (ci, c) in self.circuit.components.iter().enumerate() {
                    for (k, n) in nl.comp_nets[ci].iter().enumerate() {
                        if c.kind != Kind::Ground {
                            pins[*n].push(format!("{}.{}", c.refdes, c.kind.pins()[k].2));
                        }
                    }
                }
                egui::Grid::new("nets").striped(true).num_columns(3).show(ui, |ui| {
                    ui.strong("Net");
                    ui.strong("Voltage");
                    ui.strong("Connected pins");
                    ui.end_row();
                    for (n, p) in pins.iter().enumerate() {
                        ui.label(if n == 0 { "0 (ground)".to_string() } else { n.to_string() });
                        let v = self.sim.as_ref().filter(|_| !self.sim_board).map(|s| fmt_reading(s.v(n), "V")).unwrap_or_else(|| "—".into());
                        ui.monospace(v);
                        ui.label(p.join(", "));
                        ui.end_row();
                    }
                });
            }
            BottomTab::Components => {
                egui::Grid::new("comps").striped(true).num_columns(4).show(ui, |ui| {
                    ui.strong("RefDes");
                    ui.strong("Part");
                    ui.strong("Value");
                    ui.strong("Position");
                    ui.end_row();
                    let mut v: Vec<&Component> = self.circuit.components.iter().filter(|c| c.kind != Kind::Ground).collect();
                    v.sort_by(|a, b| natural_key(&a.refdes).cmp(&natural_key(&b.refdes)));
                    for c in v {
                        ui.label(&c.refdes);
                        ui.label(c.kind.name());
                        ui.label(c.value_label());
                        ui.label(format!("({}, {})  {}°", c.pos.x, c.pos.y, c.rot as u32 * 90));
                        ui.end_row();
                    }
                });
            }
            BottomTab::Erc => {
                let issues = erc(&self.circuit);
                if issues.is_empty() {
                    ui.label(RichText::new("No problems found.").color(Color32::from_rgb(0, 130, 40)));
                }
                for s in issues {
                    ui.label(RichText::new(format!("Warning: {}", s)).color(Color32::from_rgb(170, 90, 0)));
                }
            }
        });
    }

    pub(super) fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let hint = if self.view == View::Breadboard {
                match self.btool {
                    BTool::Jumper => if self.jumper_start.is_some() { "Click a second hole to finish the jumper · right-click or Esc cancels".to_string() } else { "Jumper wire: click a hole to start · pick a colour above".to_string() },
                    BTool::Place(k) => {
                        if crate::board::is_flying(k) && self.bbody.is_none() {
                            format!("Click a spot for the {} body, then click two holes for its leads", k.name().to_lowercase())
                        } else if crate::board::auto_pins(k, BH::new(0, 0)).is_some() {
                            format!("Click a hole to plug in the {} · it takes neighbouring holes", k.name().to_lowercase())
                        } else {
                            format!("Click {} hole for the {} · hold Shift to place several", if self.bplace.is_empty() { "the first" } else { "the second" }, k.name().to_lowercase())
                        }
                    }
                    BTool::Select => {
                        if self.running {
                            "Board powered · hold B (or the button) to press it · Space flips the switch · A / Shift+A turn the knob · F5 stops".into()
                        } else {
                            "Hover a hole to see which holes are joined · drag parts to move · double-click to edit · Del removes · press Run to power the board".into()
                        }
                    }
                }
            } else if self.wiring.is_some() {
                "Click to add a corner · click a pin or wire to connect · double-click to end · Esc cancels".to_string()
            } else {
                match self.tool {
                    Tool::Place(k) => format!("Click to place a {} · R rotates · hold Shift to place several · Esc cancels", k.name().to_lowercase()),
                    Tool::Wire => "Wire mode: click anywhere to start a wire · W or Esc to leave".into(),
                    Tool::Select => {
                        if self.running {
                            "Simulating · press a switch's key (or click it) to toggle · A / Shift+A turn potentiometers · F5 stops".into()
                        } else {
                            "Click to select · drag to move · click a pin to wire · double-click to edit · Ctrl+R rotates · Del deletes".into()
                        }
                    }
                }
            };
            ui.label(RichText::new(hint).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(sim) = &self.sim {
                    ui.label(RichText::new(format!("{}Tran: {}   step {}   {} steps/frame", if self.sim_board { "Breadboard · " } else { "" }, fmt_si(sim.t, "s"), fmt_si(self.h_used, "s"), self.steps_last_frame)).monospace().small());
                } else if let Some(e) = &self.sim_error {
                    ui.label(RichText::new(e).small().color(Color32::from_rgb(190, 40, 40)));
                } else {
                    ui.label(RichText::new("Simulation stopped").small());
                }
                if self.dirty {
                    ui.label(RichText::new("* modified").small().color(Color32::from_rgb(170, 90, 0)));
                }
            });
        });
    }
}
