use std::sync::mpsc;

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

use crate::translator::UiMsg;

pub struct TrayHandle {
    // Keep alive for the process lifetime
    _tray: TrayIcon,
}

pub fn setup_tray(ui_tx: mpsc::Sender<UiMsg>) -> TrayHandle {
    let icon = load_icon();

    let menu = Menu::new();
    let show_item = MenuItem::new("Show / Hide", true, None);
    let quit_item = MenuItem::new("Quit TensorL", true, None);
    menu.append(&show_item).unwrap();
    menu.append(&quit_item).unwrap();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("TensorL — Local Translator")
        .with_icon(icon)
        .build()
        .expect("failed to build tray icon");

    // Tray left-click → toggle window
    let tx_click = ui_tx.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click { .. } = event {
            let _ = tx_click.send(UiMsg::TrayToggle);
        }
    }));

    // Menu item events — poll on a background thread
    let tx_menu = ui_tx.clone();
    let show_id = show_item.id().clone();
    let quit_id = quit_item.id().clone();
    std::thread::Builder::new()
        .name("tensorl-tray-menu".into())
        .spawn(move || loop {
            if let Ok(ev) = MenuEvent::receiver().try_recv() {
                if ev.id == quit_id {
                    let _ = tx_menu.send(UiMsg::TrayQuit);
                } else if ev.id == show_id {
                    let _ = tx_menu.send(UiMsg::TrayToggle);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        })
        .expect("failed to spawn tray menu thread");

    TrayHandle { _tray: tray }
}

fn load_icon() -> Icon {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes)
        .expect("failed to load icon.png")
        .into_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).expect("failed to create tray icon")
}
