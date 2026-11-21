#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! OM Disk Analyzer Professional - a disk space analyzer by OM Industries Inc.
//!
//! Usage: om-disk-analyzer [folder]
//!        om-disk-analyzer --screenshot <file.png> [--tab <name>] [folder]
//!        om-disk-analyzer --setup | --uninstall   (a copy named "...Setup.exe" runs --setup)

mod app;
mod draw;
mod fmt;
mod scan;
mod settings;
mod setup;
mod sys;
mod update;

use std::path::PathBuf;

pub const APP_NAME: &str = "OM Disk Analyzer";
pub const EDITION: &str = "Professional";
pub const COMPANY: &str = "OM Industries Inc";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Launch {
    pub start: Option<String>,
    pub screenshot: Option<PathBuf>,
    pub tab: Option<String>,
    pub about: bool,
    pub options: bool,
    pub show_files: bool,
    pub setup: Option<setup::Mode>,
}

fn parse_args() -> Result<Launch, String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut take = |flag: &str| -> Result<Option<String>, String> {
        if let Some(i) = args.iter().position(|a| a == flag) {
            if i + 1 >= args.len() {
                return Err(format!("{flag} needs a value"));
            }
            let v = args.remove(i + 1);
            args.remove(i);
            Ok(Some(v))
        } else {
            Ok(None)
        }
    };
    let screenshot = take("--screenshot")?.map(PathBuf::from);
    let tab = take("--tab")?;
    let about = args.iter().any(|a| a == "--about");
    let options = args.iter().any(|a| a == "--options");
    let show_files = args.iter().any(|a| a == "--show-files");
    let named_setup = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase().contains("setup")))
        .unwrap_or(false);
    let setup = if args.iter().any(|a| a == "--uninstall") {
        Some(setup::Mode::Uninstall)
    } else if named_setup || args.iter().any(|a| a == "--setup") {
        Some(setup::Mode::Install)
    } else {
        None
    };
    args.retain(|a| !["--about", "--options", "--show-files", "--setup", "--uninstall", "--elevated"].contains(&a.as_str()));
    let mut start = args.into_iter().next().map(|s| {
        // Explorer passes drives as `C:\"` which arrives as `C:"`; repair it.
        let mut s = s.trim().trim_matches('"').to_string();
        if s.len() == 2 && s.ends_with(':') {
            s.push('\\');
        }
        s
    });
    if screenshot.is_some() && start.is_none() && setup.is_none() {
        start = Some(sample_folder());
    }
    Ok(Launch { start, screenshot, tab, about, options, show_files, setup })
}

/// A small, safe folder with real contents for the check picture.
fn sample_folder() -> String {
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = PathBuf::from(home);
        for p in [home.join(".cargo").join("registry").join("src"), home.join(".rustup").join("toolchains")] {
            if p.is_dir() {
                // step into folders that only wrap a single subfolder, so the picture shows a real tree
                let mut p = p;
                for _ in 0..4 {
                    let entries: Vec<_> = std::fs::read_dir(&p).map(|r| r.flatten().collect()).unwrap_or_default();
                    match entries.as_slice() {
                        [only] if only.path().is_dir() => p = only.path(),
                        _ => break,
                    }
                }
                return p.to_string_lossy().into_owned();
            }
        }
    }
    if !cfg!(windows) {
        return "/usr/share".into();
    }
    let win = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    format!("{win}\\Web")
}

fn main() -> eframe::Result<()> {
    let launch = match parse_args() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}\nusage: om-disk-analyzer [--screenshot <file.png>] [--tab <name>] [folder]");
            std::process::exit(2);
        }
    };
    if launch.screenshot.is_some() {
        settings::set_read_only();
    }
    if let Some(mode) = launch.setup {
        return setup::run(mode, launch.screenshot);
    }
    // An administrator's account starts the program with administrator rights (Windows asks
    // once each time). Test pictures and a copy that was already started elevated never ask.
    let already = std::env::args().any(|a| a == "--elevated");
    if launch.screenshot.is_none() && !already && settings::Settings::load().start_as_admin && sys::can_elevate() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if sys::restart_elevated(&args).is_ok() {
            return Ok(());
        }
        // permission not given: carry on without administrator rights
    }
    let icon = image::load_from_memory(include_bytes!("../assets/icon_256.png")).ok().map(|img| {
        let img = img.to_rgba8();
        let (w, h) = img.dimensions();
        egui::IconData { rgba: img.into_raw(), width: w, height: h }
    });
    let shot = launch.screenshot.is_some();
    let mut vp = egui::ViewportBuilder::default()
        .with_title(format!("{APP_NAME} {EDITION}  |  {COMPANY}"))
        .with_inner_size([1340.0, 600.0])
        .with_min_inner_size([720.0, 420.0]);
    if let Some(icon) = icon {
        vp = vp.with_icon(std::sync::Arc::new(icon));
    }
    if shot {
        vp = vp.with_active(false);
        #[cfg(windows)]
        {
            vp = vp.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let options = eframe::NativeOptions {
        viewport: vp,
        run_and_return: false,
        #[cfg(target_os = "macos")]
        event_loop_builder: if shot {
            Some(Box::new(|b| {
                use winit::platform::macos::EventLoopBuilderExtMacOS;
                b.with_activate_ignoring_other_apps(false);
            }))
        } else {
            None
        },
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, launch)))))
}
