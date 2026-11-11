#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod board;
mod draw;
mod examples;
mod instruments;
mod model;
mod sim;
mod theme;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<PathBuf> = None;
    let mut example: Option<usize> = None;
    let mut open_file: Option<PathBuf> = None;
    let mut theme: Option<String> = None;
    let mut view: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" if i + 1 < args.len() => {
                screenshot = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--view" if i + 1 < args.len() => {
                view = Some(args[i + 1].clone());
                i += 1;
            }
            "--theme" if i + 1 < args.len() => {
                theme = Some(args[i + 1].clone());
                i += 1;
            }
            "--example" if i + 1 < args.len() => {
                example = args[i + 1].parse().ok();
                i += 1;
            }
            other if !other.starts_with("--") => open_file = Some(PathBuf::from(other)),
            _ => {}
        }
        i += 1;
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Circuit Bench")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([900.0, 600.0])
        .with_icon(app_icon());
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native(
        "Circuit Bench",
        options,
        Box::new(move |cc| Box::new(app::App::new(cc, screenshot, example, open_file, theme, view))),
    )
}

/// Window / taskbar icon drawn in code: a blue tile with a resistor zigzag and an amber LED dot.
fn app_icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    let zig = |x: f32| -> f32 {
        if !(14.0..=50.0).contains(&x) {
            return 34.0;
        }
        let t = (x - 14.0) / 6.0;
        let f = t.fract();
        let v = if (t as i32) % 2 == 0 { f } else { 1.0 - f };
        26.0 + 16.0 * v
    };
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let r = 12.0;
            let dx = (fx - 32.0).abs() - (32.0 - r);
            let dy = (fy - 32.0).abs() - (32.0 - r);
            let outside = dx.max(0.0).hypot(dy.max(0.0)) - r;
            let a = (0.5 - outside).clamp(0.0, 1.0);
            let shade = 1.0 - fy / 64.0 * 0.35;
            let mut c = [(52.0 * shade) as u8, (110.0 * shade) as u8, (235.0 * shade) as u8];
            if (fy - zig(fx)).abs() < 2.4 && (4.0..=60.0).contains(&fx) {
                c = [255, 255, 255];
            }
            if (fx - 48.0).hypot(fy - 14.0) < 6.0 {
                c = [255, 200, 40];
            }
            let i = (y * n + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], (a * 255.0) as u8]);
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}
