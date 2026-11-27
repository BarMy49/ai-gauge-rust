#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod auth_client;
mod browser;
mod claude_code;
mod codex;
mod engine;
mod icon;
mod model;
mod providers;
mod store;
mod tray;
mod ui;
use anyhow::Result;
use std::{path::PathBuf, sync::Arc};

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let smoke = args.iter().any(|a| a == "--smoke-test");
    let visual = args.iter().any(|a| a == "--visual-test");
    let demo = smoke || visual || args.iter().any(|a| a == "--demo");
    let hidden = args.iter().any(|a| a == "--hidden");
    let root = args
        .windows(2)
        .find(|a| a[0] == "--data-dir")
        .map(|a| PathBuf::from(&a[1]));
    let temp = if demo {
        Some(tempfile::tempdir()?)
    } else {
        None
    };
    let store = Arc::new(store::Store::new(
        temp.as_ref().map(|p| p.path().to_path_buf()).or(root),
    )?);
    let _lock = store.lock()?;
    let mut config = store.load()?;
    if visual {
        if let Some(value) = args.windows(2).find(|a| a[0] == "--theme") {
            config.appearance = value[1].clone();
        }
        if let Some(value) = args.windows(2).find(|a| a[0] == "--text-scale") {
            config.text_scale = value[1].parse().unwrap_or(100);
        }
        config.validate()?;
    }
    let engine = engine::Engine::new(store, config, demo);
    if visual {
        if let Some(value) = args.windows(2).find(|a| a[0] == "--scenario") {
            engine.visual_fixture(&value[1]);
        }
    }
    if smoke {
        let quit = engine.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            quit.quit();
        });
    }
    ui::run(engine, hidden)?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        if std::env::args().any(|a| a == "--smoke-test" || a == "--visual-test") {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        ui::error(&format!("{error:#}"));
    }
}
