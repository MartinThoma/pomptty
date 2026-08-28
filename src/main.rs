#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod fonts;
mod history;
mod terminal;
mod ui;

use anyhow::{Context, Result};

use crate::config::Config;

fn main() -> Result<()> {
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

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([config.window.width, config.window.height])
            .with_min_inner_size([320.0, 200.0])
            .with_title("pomptty")
            .with_app_id("pomptty"),
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
