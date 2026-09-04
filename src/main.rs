#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod fonts;
mod history;
mod session;
mod terminal;
mod ui;

use std::process::ExitCode;

use anyhow::{Context, Result};

use crate::config::Config;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--print-integration") => return print_integration(args.get(1).map(String::as_str)),
        Some("--help" | "-h") => {
            print_help();
            return ExitCode::SUCCESS;
        }
        Some("--version" | "-V") => {
            println!("pomptty {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some(other) if other.starts_with('-') => {
            eprintln!("pomptty: unknown option {other:?}\n");
            print_help();
            return ExitCode::from(2);
        }
        _ => {}
    }

    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pomptty: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    export_env();

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("pomptty=info,warn"),
    )
    .init();

    let config_path = Config::config_path().context("resolving config path")?;
    let (config, config_error) = match Config::load_or_create(&config_path) {
        Ok(cfg) => (cfg, None),
        Err(e) => {
            log::error!("{e:#}");
            (Config::default(), Some(format!("{e:#}")))
        }
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([config.window.width, config.window.height])
        .with_min_inner_size([320.0, 200.0])
        .with_title("pomptty")
        .with_app_id("pomptty");
    if config.window.decorations == config::Decoration::Custom {
        // pomptty draws its own title bar / borders. The explicit "normal"
        // window type stops some X11 WMs (e.g. Marco) from auto-maximizing an
        // undecorated window.
        viewport = viewport
            .with_decorations(false)
            .with_window_type(egui::X11WindowType::Normal)
            .with_resizable(true);
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "pomptty",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(app::PompttyApp::new(
                cc,
                config,
                config_path,
                config_error,
            )?))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

/// Set up the environment that shells we spawn inherit. Called once, before any
/// thread or `eframe` starts.
///
/// `egui_term` never sets `TERM`/`COLORTERM` itself, so without this the shell
/// would inherit whatever launched pomptty (often wrong, or unset — which breaks
/// line editing, `Ctrl+R`, colors). `xterm-256color` is the safe baseline the
/// backend emulates.
fn export_env() {
    // SAFETY: `main` is still single-threaded here — nothing else reads or
    // writes the environment until `eframe::run_native` below.
    unsafe {
        std::env::set_var("TERM", "xterm-256color");
        std::env::set_var("COLORTERM", "truecolor");
        std::env::set_var("POMPTTY", "1");
    }
    if let Some(dir) = history::history_dir()
        && std::fs::create_dir_all(&dir).is_ok()
    {
        // SAFETY: see above.
        unsafe { std::env::set_var("POMPTTY_HISTORY_DIR", &dir) };
    }
}

fn print_integration(shell: Option<&str>) -> ExitCode {
    match shell.and_then(history::integration::snippet) {
        Some(snippet) => {
            print!("{snippet}");
            ExitCode::SUCCESS
        }
        None => {
            eprintln!(
                "pomptty --print-integration: expected one of: {}",
                history::integration::SUPPORTED.join(", "),
            );
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    let shells = history::integration::SUPPORTED.join("|");
    println!(
        "pomptty {ver} — a minimal, Chrome-flavored terminal emulator\n\
         \n\
         Usage:\n  \
         pomptty                             launch the terminal\n  \
         pomptty --print-integration <{shells}>   print the shell history hook\n  \
         pomptty --help                      show this help\n  \
         pomptty --version                   show the version\n\
         \n\
         Enable command history by adding the hook to your shell rc, e.g.\n  \
         eval \"$(pomptty --print-integration bash)\"",
        ver = env!("CARGO_PKG_VERSION"),
    );
}
