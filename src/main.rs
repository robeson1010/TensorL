// Hide the console window in release builds (also prevents Ctrl+C / SIGINT
// from conflicting with our global hotkey registration).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod hotkey;
mod translator;

use std::sync::mpsc;

use config::{config_path, AppConfig};
use translator::{InferRequest, UiMsg};

#[cfg(windows)]
fn acquire_single_instance_lock() -> Option<windows::Win32::Foundation::HANDLE> {
    use windows::core::w;
    use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
    use windows::Win32::System::Threading::CreateMutexW;

    let handle = unsafe { CreateMutexW(None, true, w!("Global\\TensorL_SingleInstance")).ok()? };
    let already_running = unsafe { windows::Win32::Foundation::GetLastError() } == ERROR_ALREADY_EXISTS;
    if already_running {
        return None;
    }
    Some(handle)
}

fn main() -> eframe::Result<()> {
    #[cfg(windows)]
    let _mutex = match acquire_single_instance_lock() {
        Some(h) => h,
        None => {
            // Another instance is already running — exit silently.
            std::process::exit(0);
        }
    };
    // Logging (visible in debug builds because windows_subsystem is not set)
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Load or create config
    let config = AppConfig::load(&config_path());

    // Communication channels
    let (ui_tx, ui_rx) = mpsc::channel::<UiMsg>();
    let (infer_tx, infer_rx) = mpsc::channel::<InferRequest>();

    // Spawn background threads
    hotkey::spawn_hotkey_thread(ui_tx.clone());
    translator::spawn_inference_thread(config.clone(), infer_rx, ui_tx);

    // Build window icon from embedded PNG
    let icon = {
        let bytes = include_bytes!("../assets/icon.png");
        eframe::icon_data::from_png_bytes(bytes).unwrap_or_else(|_| {
            let img = image::load_from_memory(bytes)
                .expect("failed to load icon.png")
                .into_rgba8();
            let (w, h) = img.dimensions();
            egui::IconData {
                rgba: img.into_raw(),
                width: w,
                height: h,
            }
        })
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TensorL")
            .with_decorations(false)
            .with_inner_size([880.0, 520.0])
            .with_min_inner_size([600.0, 380.0])
            .with_icon(std::sync::Arc::new(icon))
            .with_taskbar(true),
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "TensorL",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(app::TensorLApp::new(
                cc,
                ui_rx,
                infer_tx,
                config,
            )))
        }),
    )
}
