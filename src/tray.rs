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

// ── Shared HWND for direct window show from tray thread ──────────────────────

#[cfg(target_os = "windows")]
static SHARED_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

#[cfg(target_os = "windows")]
pub fn set_shared_hwnd(hwnd: windows::Win32::Foundation::HWND) {
    SHARED_HWND.store(hwnd.0 as isize, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_os = "windows")]
fn force_show_window() {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
        SetForegroundWindow, ShowWindow, SW_SHOW, SW_RESTORE,
    };

    extern "system" {
        fn GetCurrentThreadId() -> u32;
        fn AttachThreadInput(attach: u32, attach_to: u32, do_attach: i32) -> i32;
    }

    let raw = SHARED_HWND.load(std::sync::atomic::Ordering::Relaxed);
    if raw == 0 { return; }
    let hwnd = HWND(raw as *mut std::ffi::c_void);

    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        ShowWindow(hwnd, SW_RESTORE);

        let fg_hwnd = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg_hwnd, None);
        let our_thread = GetCurrentThreadId();

        if fg_thread != our_thread && fg_thread != 0 {
            AttachThreadInput(our_thread, fg_thread, 1);
            let _ = SetForegroundWindow(hwnd);
            AttachThreadInput(our_thread, fg_thread, 0);
        } else {
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_shared_hwnd(_: ()) {}

// ── Setup ────────────────────────────────────────────────────────────────────

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

    // Tray left-click → show window directly + send toggle
    let tx_click = ui_tx.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click { .. } = event {
            #[cfg(target_os = "windows")]
            force_show_window();
            let _ = tx_click.send(UiMsg::TrayToggle);
        }
    }));

    // Menu item events
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
                    #[cfg(target_os = "windows")]
                    force_show_window();
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
