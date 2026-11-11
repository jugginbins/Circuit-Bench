//! Floating windows: part properties, wire colour, instrument panels, analyses, grapher, files.

use super::util::*;
use super::*;

impl App {
    pub(super) fn props_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.props else {
            return;
        };
        let mut open = true;
        let mut close = false;
        let mut cancel = false;
        if id == 0 {
            egui::Window::new("Circuit title").collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
                ui.text_edit_singleline(&mut self.circuit.title);
                if ui.button("OK").clicked() {
                    close = true;
                }
            });
        } else {
            let Some(title) = self.circuit.comp_any(id).map(|c| format!("{} — {}", c.kind.name(), c.refdes)) else {
                self.props = None;
                return;
            };
            egui::Window::new(title).id(Id::new("props_win")).collapsible(false).resizable(false).default_width(320.0).open(&mut open).show(ctx, |ui| {
                let Some(c) = self.circuit.comp_any_mut(id) else {
                    return;
                };
                let pid = Id::new(("prop", id));
                egui::Grid::new("props_grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    ui.label("RefDes");
                    ui.add(egui::TextEdit::singleline(&mut c.refdes).desired_width(150.0));
                    ui.end_row();
                    super::props::part_fields(ui, c.kind, &mut c.params, pid);
                    ui.label("Rotation");
                    ui.label(format!("{}°", c.rot as u32 * 90));
                    ui.end_row();
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("  OK  ").clicked() {
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
                ui.label(RichText::new("Values accept prefixes: p n u m k M (e.g. 4.7k, 100n, 2.2m).").small().weak());
            });
        }
        if cancel {
            if let Some(o) = self.props_orig.take() {
                self.circuit = o;
            }
            self.props = None;
        } else if close || !open {
            if let Some(o) = self.props_orig.take() {
                let changed = serde_json::to_string(&o).ok() != serde_json::to_string(&self.circuit).ok();
                if changed {
                    self.push_undo(o);
                }
            }
            self.props = None;
        }
    }

    pub(super) fn wire_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.wire_props else {
            return;
        };
        let Some(idx) = self.circuit.wires.iter().position(|w| w.id == id) else {
            self.wire_props = None;
            return;
        };
        let mut open = true;
        let mut delete = false;
        egui::Window::new("Wire").collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.label("Colour (also used for oscilloscope traces):");
            ui.horizontal(|ui| {
                for c in WireColor::ALL {
                    let [r, g, b] = c.rgb();
                    let sel = self.circuit.wires[idx].color == c;
                    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 22.0), Sense::click());
                    ui.painter().rect(rect.shrink(2.0), 3.0, Color32::from_rgb(r, g, b), Stroke::new(if sel { 2.5 } else { 0.5 }, Color32::BLACK));
                    if resp.on_hover_text(c.name()).clicked() {
                        self.circuit.wires[idx].color = c;
                        self.dirty = true;
                    }
                }
            });
            if ui.button("Delete wire").clicked() {
                delete = true;
            }
        });
        if delete {
            self.edit_stop();
            self.checkpoint();
            self.circuit.wires.remove(idx);
            self.wire_props = None;
        } else if !open {
            self.wire_props = None;
        }
    }

    pub(super) fn instrument_windows(&mut self, ctx: &egui::Context) {
        let ids = self.open_inst.clone();
        let mut screen = ctx.screen_rect();
        screen.min.y += 70.0;
        for (n, id) in ids.iter().enumerate() {
            let Some(idx) = self.circuit.components.iter().position(|c| c.id == *id) else {
                continue;
            };
            let kind = self.circuit.components[idx].kind;
            let refdes = self.circuit.components[idx].refdes.clone();
            let mut open = true;
            let default_pos = match kind {
                Kind::Oscilloscope => pos2(screen.right() - 790.0, screen.top() + 24.0 + n as f32 * 30.0),
                _ => pos2(screen.right() - 540.0 + n as f32 * 30.0, screen.top() + 24.0 + n as f32 * 30.0),
            };
            let title = format!("{} — {}", kind.name(), refdes);
            let mut structural = false;
            let mut open_settings = false;
            let mut bode_run = false;
            egui::Window::new(title)
                .id(Id::new(("inst", *id)))
                .default_pos(default_pos)
                .resizable(false)
                .default_width(match kind {
                    Kind::Oscilloscope => 520.0,
                    Kind::BodePlotter => 480.0,
                    _ => 280.0,
                })
                .open(&mut open)
                .show(ctx, |ui| {
                    if !matches!(kind, Kind::Oscilloscope | Kind::Wattmeter) && ui.small_button("Settings…").clicked() {
                        open_settings = true;
                    }
                    match kind {
                    Kind::Oscilloscope => {
                        let colors = scope_colors(&self.circuit, idx);
                        let sc = self.scopes.entry(*id).or_default();
                        instruments::scope_panel(ui, sc, colors);
                    }
                    Kind::Multimeter => {
                        let reading = self.meters.get(id).map(|m| multimeter_text(&self.circuit.components[idx], m)).unwrap_or_else(|| "----".into());
                        let p = &mut self.circuit.components[idx].params;
                        if instruments::multimeter_panel(ui, p, &reading) {
                            structural = true;
                        }
                    }
                    Kind::FuncGen => {
                        let p = &mut self.circuit.components[idx].params;
                        if instruments::funcgen_panel(ui, Id::new(("fg", *id)), p) {
                            self.dirty = true;
                        }
                        instruments::wave_preview(ui, p);
                    }
                    Kind::Wattmeter => {
                        let pw = self.meters.get(id).map(|m| m.mean()).unwrap_or(0.0);
                        let rms = |m: Option<&Meter>| m.map(|m| (m.mean().powi(2) + m.ac_rms().powi(2)).sqrt()).unwrap_or(0.0);
                        let vr = rms(self.meters.get(&sub_id(*id, 1)));
                        let ir = rms(self.meters.get(&sub_id(*id, 2)));
                        let pf = if vr * ir > 1e-15 { (pw / (vr * ir)).clamp(-1.0, 1.0) } else { 0.0 };
                        instruments::wattmeter_panel(ui, pw, pf, vr, ir);
                    }
                    Kind::FreqCounter => {
                        let stats = self.meters.get(id).and_then(|m| instruments::freq_stats(m, self.circuit.components[idx].params.value));
                        let p = &mut self.circuit.components[idx].params;
                        instruments::freq_counter_panel(ui, Id::new(("fc", *id)), p, stats);
                    }
                    Kind::BodePlotter => {
                        let st = self.bodes.entry(*id).or_insert_with(|| BodeState { ppd: 30, ..Default::default() });
                        let p = &mut self.circuit.components[idx].params;
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut st.show_phase, false, "Magnitude");
                            ui.selectable_value(&mut st.show_phase, true, "Phase");
                            ui.separator();
                            ui.label("From");
                            if instruments::si_field(ui, Id::new(("bf0", *id)), &mut p.freq, "Hz", 70.0) {
                                bode_run = true;
                            }
                            ui.label("to");
                            if instruments::si_field(ui, Id::new(("bf1", *id)), &mut p.f_stop, "Hz", 70.0) {
                                bode_run = true;
                            }
                        });
                        let (rect, _) = ui.allocate_exact_size(vec2(460.0, 230.0), Sense::hover());
                        let mut child = ui.child_ui(rect, egui::Layout::top_down(egui::Align::Min));
                        let data = if st.show_phase { st.phase.clone() } else { st.mag.clone() };
                        let unit = if st.show_phase { "°" } else { " dB" };
                        let col = pal().accent;
                        egui_plot::Plot::new(("bode", *id))
                            .height(230.0)
                            .width(460.0)
                            .allow_scroll(false)
                            .y_axis_label(if st.show_phase { "Phase (°)" } else { "Gain (dB)" })
                            .x_axis_formatter(log_freq_fmt)
                            .label_formatter(move |_, pt| format!("{}\n{:.2}{}", fmt_si(10f64.powf(pt.x), "Hz"), pt.y, unit))
                            .show(&mut child, |pu| {
                                pu.line(egui_plot::Line::new(egui_plot::PlotPoints::from(data)).color(col).width(2.0));
                            });
                        if let Some(e) = &st.err {
                            ui.colored_label(Color32::from_rgb(220, 90, 60), e);
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Analyze").clicked() {
                                bode_run = true;
                            }
                            ui.label("Points / decade");
                            if ui.add(egui::DragValue::new(&mut st.ppd).clamp_range(5..=200)).changed() {
                                bode_run = true;
                            }
                        });
                        if st.mag.is_empty() && st.err.is_none() {
                            bode_run = true;
                        }
                    }
                    _ => {}
                    }
                });
            if structural {
                self.dirty = true;
                self.meters.remove(id);
            }
            if bode_run || (kind == Kind::BodePlotter && self.running && self.frame % 45 == 0) {
                let (f0, f1) = (self.circuit.components[idx].params.freq, self.circuit.components[idx].params.f_stop);
                let ppd = self.bodes.get(id).map(|b| b.ppd).unwrap_or(30).max(5);
                let res = sim::run_bode(&self.circuit, idx, f0, f1, ppd);
                let st = self.bodes.entry(*id).or_default();
                st.computed_at = self.frame;
                match res {
                    Ok((m, ph)) => {
                        st.mag = m;
                        st.phase = ph;
                        st.err = None;
                    }
                    Err(e) => st.err = Some(e),
                }
            }
            if open_settings {
                self.props = Some(*id);
                self.props_orig = Some(self.circuit.clone());
            }
            if !open {
                self.open_inst.retain(|x| x != id);
            }
        }
    }

    pub(super) fn analysis_window(&mut self, ctx: &egui::Context) {
        if !self.analysis_open {
            return;
        }
        let mut open = true;
        let mut run = false;
        egui::Window::new("Analyses and simulation").default_width(620.0).open(&mut open).show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(170.0);
                    ui.label(RichText::new("Active analysis").strong());
                    for (k, n) in [
                        (AnalysisKind::Interactive, "Interactive simulation"),
                        (AnalysisKind::DcOp, "DC operating point"),
                        (AnalysisKind::Ac, "AC sweep"),
                        (AnalysisKind::Transient, "Transient"),
                        (AnalysisKind::DcSweep, "DC sweep"),
                    ] {
                        ui.selectable_value(&mut self.cfg.sel, k, n);
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    self.analysis_settings(ui);
                    if matches!(self.cfg.sel, AnalysisKind::Ac | AnalysisKind::Transient | AnalysisKind::DcSweep) {
                        ui.separator();
                        self.output_vars(ui);
                    }
                    ui.add_space(8.0);
                    let label = if self.cfg.sel == AnalysisKind::Interactive { "Run interactive" } else { "Run" };
                    if ui.add(egui::Button::new(RichText::new(label).strong()).min_size(vec2(90.0, 26.0))).clicked() {
                        run = true;
                    }
                });
            });
        });
        if !open {
            self.analysis_open = false;
        }
        if run {
            self.run_analysis();
        }
    }

    fn analysis_settings(&mut self, ui: &mut Ui) {
        let gid = Id::new("an");
        let sources: Vec<(u64, String)> = self.circuit.components.iter().filter(|c| matches!(c.kind, Kind::DcVoltage | Kind::DcCurrent)).map(|c| (c.id, c.refdes.clone())).collect();
        let cfg = &mut self.cfg;
        match cfg.sel {
            AnalysisKind::Interactive => {
                ui.label(RichText::new("Interactive simulation settings").strong());
                ui.label("Runs the circuit live in real time; instruments and indicators update as it runs.");
                egui::Grid::new("isg").num_columns(2).show(ui, |ui| {
                    ui.label("Maximum time step (0 = automatic)");
                    instruments::si_field(ui, gid.with("ms"), &mut cfg.max_step, "s", 120.0);
                    ui.end_row();
                    ui.label("Initial conditions");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut cfg.ic_op, false, "Set to zero");
                        ui.selectable_value(&mut cfg.ic_op, true, "DC operating point");
                    });
                    ui.end_row();
                });
            }
            AnalysisKind::DcOp => {
                ui.label(RichText::new("DC operating point").strong());
                ui.label("Capacitors act as open circuits and inductors as short circuits; AC sources use their DC offset. Results list every node voltage and branch current.");
            }
            AnalysisKind::Ac => {
                ui.label(RichText::new("AC sweep").strong());
                egui::Grid::new("acg").num_columns(2).show(ui, |ui| {
                    ui.label("Start frequency");
                    instruments::si_field(ui, gid.with("f0"), &mut cfg.ac_fstart, "Hz", 120.0);
                    ui.end_row();
                    ui.label("Stop frequency");
                    instruments::si_field(ui, gid.with("f1"), &mut cfg.ac_fstop, "Hz", 120.0);
                    ui.end_row();
                    ui.label("Points per decade");
                    ui.add(egui::DragValue::new(&mut cfg.ac_ppd).clamp_range(2..=200));
                    ui.end_row();
                });
                ui.label(RichText::new("AC voltage sources and function generators drive the sweep with their amplitude.").small().weak());
            }
            AnalysisKind::Transient => {
                ui.label(RichText::new("Transient").strong());
                egui::Grid::new("trg").num_columns(2).show(ui, |ui| {
                    ui.label("End time");
                    instruments::si_field(ui, gid.with("t1"), &mut cfg.tr_stop, "s", 120.0);
                    ui.end_row();
                    ui.label("Time step");
                    instruments::si_field(ui, gid.with("ts"), &mut cfg.tr_step, "s", 120.0);
                    ui.end_row();
                    ui.label("Initial conditions");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut cfg.tr_use_op, false, "Set to zero");
                        ui.selectable_value(&mut cfg.tr_use_op, true, "DC operating point");
                    });
                    ui.end_row();
                });
            }
            AnalysisKind::DcSweep => {
                ui.label(RichText::new("DC sweep").strong());
                if cfg.dc_src.map(|id| !sources.iter().any(|s| s.0 == id)).unwrap_or(true) {
                    cfg.dc_src = sources.first().map(|s| s.0);
                }
                egui::Grid::new("dcg").num_columns(2).show(ui, |ui| {
                    ui.label("Source");
                    let cur = sources.iter().find(|s| Some(s.0) == cfg.dc_src).map(|s| s.1.clone()).unwrap_or_else(|| "(no DC source)".into());
                    egui::ComboBox::from_id_source("dcsrc").selected_text(cur).show_ui(ui, |ui| {
                        for (id, n) in &sources {
                            ui.selectable_value(&mut cfg.dc_src, Some(*id), n);
                        }
                    });
                    ui.end_row();
                    ui.label("Start value");
                    instruments::si_field(ui, gid.with("d0"), &mut cfg.dc_start, "", 120.0);
                    ui.end_row();
                    ui.label("Stop value");
                    instruments::si_field(ui, gid.with("d1"), &mut cfg.dc_stop, "", 120.0);
                    ui.end_row();
                    ui.label("Increment");
                    instruments::si_field(ui, gid.with("di"), &mut cfg.dc_incr, "", 120.0);
                    ui.end_row();
                });
            }
        }
    }

    fn output_vars(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Output variables").strong());
        let nl = sim::build_netlist(&self.circuit);
        let vars = sim::all_vars(&self.circuit, &nl);
        ui.horizontal(|ui| {
            if ui.small_button("All voltages").clicked() {
                self.cfg.out_vars = vars.iter().filter(|v| matches!(v, Var::Node(_))).map(|v| sim::var_name(&self.circuit, v)).collect();
            }
            if ui.small_button("None").clicked() {
                self.cfg.out_vars.clear();
            }
        });
        egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for v in &vars {
                    let name = sim::var_name(&self.circuit, v);
                    let mut on = self.cfg.out_vars.contains(&name);
                    if ui.checkbox(&mut on, &name).changed() {
                        if on {
                            self.cfg.out_vars.insert(name);
                        } else {
                            self.cfg.out_vars.remove(&name);
                        }
                    }
                }
            });
        });
        if self.cfg.out_vars.is_empty() {
            ui.label(RichText::new("No variables chosen: all node voltages will be plotted.").small().weak());
        }
    }

    fn chosen_vars(&self) -> Vec<Var> {
        let nl = sim::build_netlist(&self.circuit);
        let vars = sim::all_vars(&self.circuit, &nl);
        let chosen: Vec<Var> = vars.iter().filter(|v| self.cfg.out_vars.contains(&sim::var_name(&self.circuit, v))).cloned().collect();
        if chosen.is_empty() {
            vars.into_iter().filter(|v| matches!(v, Var::Node(_))).collect()
        } else {
            chosen
        }
    }

    pub(super) fn run_analysis(&mut self) {
        let title = if self.circuit.title.is_empty() { "Untitled".to_string() } else { self.circuit.title.clone() };
        let vars = self.chosen_vars();
        let res: Result<GraphTab, String> = match self.cfg.sel {
            AnalysisKind::Interactive => {
                self.analysis_open = false;
                if self.running {
                    self.stop_sim();
                }
                self.start_sim();
                return;
            }
            AnalysisKind::DcOp => sim::run_dc_op(&self.circuit).map(|r| GraphTab::Op { title: format!("{} — DC operating point", title), rows: r.rows }),
            AnalysisKind::Ac => sim::run_ac(&self.circuit, self.cfg.ac_fstart, self.cfg.ac_fstop, self.cfg.ac_ppd, &vars).map(|s| GraphTab::Ac { title: format!("{} — AC sweep", title), series: s }),
            AnalysisKind::Transient => {
                if self.cfg.tr_step <= 0.0 || self.cfg.tr_stop <= 0.0 {
                    Err("End time and time step must be greater than zero.".into())
                } else {
                    sim::run_transient(&self.circuit, self.cfg.tr_stop, self.cfg.tr_step, self.cfg.tr_use_op, &vars)
                        .map(|s| GraphTab::Xy { title: format!("{} — Transient", title), xlabel: "Time".into(), xunit: "s", series: s })
                }
            }
            AnalysisKind::DcSweep => {
                let idx = self.cfg.dc_src.and_then(|id| self.circuit.components.iter().position(|c| c.id == id));
                match idx {
                    None => Err("Choose a DC source to sweep.".into()),
                    Some(i) => {
                        let unit = if self.circuit.components[i].kind == Kind::DcCurrent { "A" } else { "V" };
                        let name = self.circuit.components[i].refdes.clone();
                        sim::run_dc_sweep(&self.circuit, i, self.cfg.dc_start, self.cfg.dc_stop, self.cfg.dc_incr, &vars)
                            .map(|s| GraphTab::Xy { title: format!("{} — DC sweep of {}", title, name), xlabel: name.clone(), xunit: unit, series: s })
                    }
                }
            }
        };
        match res {
            Ok(tab) => {
                self.log(format!("Analysis finished: {}", tab.title()));
                if let GraphTab::Op { rows, .. } = &tab {
                    for (n, v, u) in rows {
                        self.messages.push(format!("    {:<12} {}", n, fmt_reading(*v, u)));
                    }
                }
                self.graphs.push(tab);
                self.graph_sel = self.graphs.len() - 1;
                self.grapher_open = true;
            }
            Err(e) => {
                self.log(format!("Analysis failed: {}", e));
                self.bottom_tab = BottomTab::Results;
            }
        }
    }

    pub(super) fn grapher_window(&mut self, ctx: &egui::Context) {
        if !self.grapher_open {
            return;
        }
        let mut open = true;
        let mut remove: Option<usize> = None;
        let mut export: Option<usize> = None;
        egui::Window::new("Grapher").default_size([720.0, 480.0]).default_pos(pos2(260.0, 120.0)).open(&mut open).show(ctx, |ui| {
            if self.graphs.is_empty() {
                ui.label("No results yet. Use Simulate ▸ Analyses and simulation… to run an analysis.");
                return;
            }
            ui.horizontal_wrapped(|ui| {
                for (i, g) in self.graphs.iter().enumerate() {
                    ui.selectable_value(&mut self.graph_sel, i, g.title());
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Export CSV").clicked() {
                    export = Some(self.graph_sel);
                }
                if ui.button("Close tab").clicked() {
                    remove = Some(self.graph_sel);
                }
            });
            ui.separator();
            let sel = self.graph_sel.min(self.graphs.len() - 1);
            match &self.graphs[sel] {
                GraphTab::Op { rows, .. } => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("opgrid").striped(true).num_columns(2).min_col_width(140.0).show(ui, |ui| {
                            ui.strong("Variable");
                            ui.strong("Operating point value");
                            ui.end_row();
                            for (n, v, u) in rows {
                                ui.monospace(n);
                                ui.monospace(fmt_reading(*v, u));
                                ui.end_row();
                            }
                        });
                    });
                }
                GraphTab::Xy { xlabel, xunit, series, .. } => {
                    let xu = *xunit;
                    egui_plot::Plot::new(("xy", sel))
                        .legend(egui_plot::Legend::default())
                        .x_axis_label(format!("{} ({})", xlabel, xu))
                        .x_axis_formatter(move |m, _, _| fmt_si(m.value, xu))
                        .show(ui, |pu| {
                            for (k, s) in series.iter().enumerate() {
                                pu.line(egui_plot::Line::new(egui_plot::PlotPoints::from(s.pts.clone())).name(&s.name).color(palette(k)).width(1.6));
                            }
                        });
                }
                GraphTab::Ac { series, .. } => {
                    let h = (ui.available_height() - 10.0) / 2.0;
                    egui_plot::Plot::new(("acm", sel))
                        .height(h)
                        .legend(egui_plot::Legend::default())
                        .y_axis_label("Magnitude (dB)")
                        .x_axis_formatter(log_freq_fmt)
                        .link_axis("ac", true, false)
                        .label_formatter(|n, p| format!("{}\n{}  {:.2} dB", n, fmt_si(10f64.powf(p.x), "Hz"), p.y))
                        .show(ui, |pu| {
                            for (k, s) in series.iter().enumerate() {
                                pu.line(egui_plot::Line::new(egui_plot::PlotPoints::from(s.mag.clone())).name(&s.name).color(palette(k)).width(1.6));
                            }
                        });
                    egui_plot::Plot::new(("acp", sel))
                        .height(h)
                        .y_axis_label("Phase (°)")
                        .x_axis_label("Frequency")
                        .x_axis_formatter(log_freq_fmt)
                        .link_axis("ac", true, false)
                        .label_formatter(|n, p| format!("{}\n{}  {:.1}°", n, fmt_si(10f64.powf(p.x), "Hz"), p.y))
                        .show(ui, |pu| {
                            for (k, s) in series.iter().enumerate() {
                                pu.line(egui_plot::Line::new(egui_plot::PlotPoints::from(s.phase.clone())).name(&s.name).color(palette(k)).width(1.6));
                            }
                        });
                }
            }
        });
        if let Some(i) = export {
            let path = self.file_path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(default_dir).join(format!("analysis-{}.csv", i + 1));
            match std::fs::write(&path, graph_csv(&self.graphs[i])) {
                Ok(_) => self.log(format!("Exported {}", path.display())),
                Err(e) => self.log(format!("Export failed: {}", e)),
            }
        }
        if let Some(i) = remove {
            self.graphs.remove(i);
            self.graph_sel = self.graph_sel.saturating_sub(1);
        }
        if !open {
            self.grapher_open = false;
        }
    }

    pub(super) fn file_window(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.file_dlg else {
            return;
        };
        let mut open = true;
        let mut action: Option<PathBuf> = None;
        let mut cancel = false;
        let title = if dlg.netlist {
            "Export SPICE netlist"
        } else if dlg.save {
            "Save circuit"
        } else {
            "Open circuit"
        };
        egui::Window::new(title).collapsible(false).default_width(480.0).open(&mut open).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Up").clicked() {
                    if let Some(p) = dlg.dir.parent() {
                        dlg.dir = p.to_path_buf();
                    }
                }
                if ui.button("Documents").clicked() {
                    dlg.dir = default_dir();
                }
                ui.label(RichText::new(dlg.dir.display().to_string()).monospace().small());
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, false]).show(ui, |ui| {
                let mut entries: Vec<(bool, String)> = std::fs::read_dir(&dlg.dir)
                    .map(|rd| {
                        rd.filter_map(|e| e.ok())
                            .filter_map(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                if name.starts_with('.') {
                                    return None;
                                }
                                let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                                let ext_ok = if dlg.netlist { name.ends_with(".cir") } else { name.ends_with(&format!(".{}", FILE_EXT)) || name.ends_with(".json") };
                                if is_dir || ext_ok {
                                    Some((is_dir, name))
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
                for (is_dir, name) in entries {
                    let label = if is_dir { format!("[folder]  {}", name) } else { format!("    {}", name) };
                    let r = ui.selectable_label(dlg.name == name, label);
                    if r.clicked() {
                        if is_dir {
                            dlg.dir = dlg.dir.join(&name);
                        } else {
                            dlg.name = name.clone();
                        }
                    }
                    if r.double_clicked() && !is_dir {
                        action = Some(dlg.dir.join(&name));
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("File name");
                ui.add(egui::TextEdit::singleline(&mut dlg.name).desired_width(280.0));
            });
            if let Some(e) = &dlg.err {
                ui.colored_label(Color32::RED, e);
            }
            ui.horizontal(|ui| {
                let label = if dlg.save { "Save" } else { "Open" };
                if ui.button(label).clicked() && !dlg.name.trim().is_empty() {
                    let mut name = dlg.name.trim().to_string();
                    if dlg.save && !dlg.netlist && !name.contains('.') {
                        name = format!("{}.{}", name, FILE_EXT);
                    }
                    if dlg.netlist && !name.ends_with(".cir") {
                        name = format!("{}.cir", name);
                    }
                    action = Some(dlg.dir.join(name));
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if let Some(p) = action {
            let (save, netlist) = (dlg.save, dlg.netlist);
            self.file_dlg = None;
            if netlist {
                match std::fs::write(&p, spice_netlist(&self.circuit)) {
                    Ok(_) => self.log(format!("Exported netlist {}", p.display())),
                    Err(e) => self.log(format!("Export failed: {}", e)),
                }
            } else if save {
                if let Err(e) = self.save_file(&p) {
                    self.log(format!("Save failed: {}", e));
                }
            } else {
                self.load_file(&p);
            }
            return;
        }
        if cancel || !open {
            self.file_dlg = None;
        }
    }

    pub(super) fn misc_windows(&mut self, ctx: &egui::Context) {
        if let Some(choice) = self.confirm_new {
            if !self.dirty {
                self.confirm_new = None;
                self.new_circuit(choice);
            } else {
                let mut keep = true;
                egui::Window::new("Discard changes?").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
                    ui.label("The current circuit has unsaved changes.");
                    ui.horizontal(|ui| {
                        if ui.button("Discard and continue").clicked() {
                            self.new_circuit(choice);
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                });
                if !keep {
                    self.confirm_new = None;
                }
            }
        }
        if self.about_open {
            egui::Window::new("About Circuit Bench").collapsible(false).resizable(false).open(&mut self.about_open).show(ctx, |ui| {
                ui.heading("Circuit Bench");
                ui.label("A schematic capture and circuit simulation workbench.");
                ui.label("Draw circuits from a parts bin, wire them, and test them live with a multimeter, function generator and oscilloscope — or run DC, AC and transient analyses and read the results in the grapher.");
                ui.label(RichText::new("Engine: modified nodal analysis with Newton–Raphson for diodes and trapezoidal integration.").small().weak());
            });
        }
        if self.help_open {
            egui::Window::new("Quick guide").default_width(460.0).open(&mut self.help_open).show(ctx, |ui| {
                for (h, t) in [
                    ("Place parts", "Click a part in the Parts bin (left) or an instrument (right), then click the sheet. Press R to rotate before placing; hold Shift to place several."),
                    ("Wire", "Hover a pin until the red circle appears, click, then click where the wire should turn and click another pin or wire to finish. Double-click ends a wire anywhere. Esc cancels."),
                    ("Edit", "Drag parts to move them (wires follow). Double-click a part for its properties, or a wire to change its colour. Ctrl+R rotates, Del deletes, Ctrl+Z undoes."),
                    ("Ground", "Every circuit needs at least one Ground part connected to it."),
                    ("Simulate", "Press the green Run button (F5). Meters, probes, LEDs and lamps update live. Switches toggle with their key (Space by default) or a click; potentiometers turn with A / Shift+A."),
                    ("Instruments", "Double-click the multimeter, function generator or oscilloscope to open its panel. The oscilloscope shows channels A and B relative to its G pin (or ground when G is open)."),
                    ("Analyses", "Simulate ▸ Analyses runs DC operating point, AC sweep, transient and DC sweep. Results appear in the Grapher, where you can export CSV."),
                ] {
                    ui.label(RichText::new(h).strong());
                    ui.label(t);
                    ui.add_space(4.0);
                }
            });
        }
    }
}
