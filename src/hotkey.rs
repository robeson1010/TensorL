use std::sync::mpsc;
use std::time::Instant;

use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use keyboard_types::{Code, Modifiers};

use crate::translator::UiMsg;

const DOUBLE_PRESS_MS: u128 = 500;

pub fn spawn_hotkey_thread(ui_tx: mpsc::Sender<UiMsg>) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("tensorl-hotkey".into())
        .spawn(move || run_hotkey_loop(ui_tx))
        .expect("failed to spawn hotkey thread")
}

fn run_hotkey_loop(ui_tx: mpsc::Sender<UiMsg>) {
    // GlobalHotKeyManager must be created on the same thread as the message pump
    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(e) => {
            log::error!("GlobalHotKeyManager::new failed: {e}");
            return;
        }
    };

    let hotkey = HotKey::new(Some(Modifiers::CONTROL), Code::KeyC);
    if let Err(e) = manager.register(hotkey) {
        log::error!("hotkey register failed: {e}");
        return;
    }

    let receiver = GlobalHotKeyEvent::receiver();
    let mut last_fire: Option<Instant> = None;

    // On Windows, global-hotkey requires a Win32 message pump on this thread.
    // We run our own minimal pump so WM_HOTKEY messages are delivered.
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, TranslateMessage, MSG,
        };

        loop {
            // Drain any hotkey events that arrived
            while let Ok(event) = receiver.try_recv() {
                if event.id() == hotkey.id() && event.state() == HotKeyState::Released {
                    let now = Instant::now();
                    if let Some(prev) = last_fire {
                        if now.duration_since(prev).as_millis() < DOUBLE_PRESS_MS {
                            // Double Ctrl+C detected
                            let _ = ui_tx.send(UiMsg::HotkeyFired);
                            last_fire = None;
                            continue;
                        }
                    }
                    last_fire = Some(now);
                }
            }

            // Blocking Win32 message pump — wakes on WM_HOTKEY
            let mut msg = MSG::default();
            unsafe {
                if GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }

    // Non-Windows fallback (should not be reached on Windows builds)
    #[cfg(not(target_os = "windows"))]
    loop {
        while let Ok(event) = receiver.try_recv() {
            if event.id() == hotkey.id() && event.state() == HotKeyState::Released {
                let now = Instant::now();
                if let Some(prev) = last_fire {
                    if now.duration_since(prev).as_millis() < DOUBLE_PRESS_MS {
                        let _ = ui_tx.send(UiMsg::HotkeyFired);
                        last_fire = None;
                        continue;
                    }
                }
                last_fire = Some(now);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
