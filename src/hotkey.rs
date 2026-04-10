use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;

use crate::translator::UiMsg;

const DOUBLE_PRESS_MS: i64 = 500;

// Shared state accessed from the hook callback (which must be a bare extern fn)
static LAST_C_TIME_MS: AtomicI64 = AtomicI64::new(0);
static UI_TX: Mutex<Option<mpsc::Sender<UiMsg>>> = Mutex::new(None);

pub fn spawn_hotkey_thread(ui_tx: mpsc::Sender<UiMsg>) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("tensorl-hotkey".into())
        .spawn(move || run_hotkey_loop(ui_tx))
        .expect("failed to spawn hotkey thread")
}

// ── Windows implementation ────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn run_hotkey_loop(ui_tx: mpsc::Sender<UiMsg>) {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, WH_KEYBOARD_LL, MSG,
    };

    *UI_TX.lock().unwrap() = Some(ui_tx);

    unsafe {
        // WH_KEYBOARD_LL observes key events but does NOT consume them.
        // Ctrl+C continues to work normally in all other applications.
        let _hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0)
            .expect("failed to install low-level keyboard hook");

        // Win32 message pump — required for WH_KEYBOARD_LL delivery
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_hook_proc(
    n_code: i32,
    w_param: windows::Win32::Foundation::WPARAM,
    l_param: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, KBDLLHOOKSTRUCT, WM_KEYDOWN,
    };

    if n_code >= 0 && w_param.0 as u32 == WM_KEYDOWN {
        let kbd = &*(l_param.0 as *const KBDLLHOOKSTRUCT);

        // vkCode 0x43 = 'C'
        if kbd.vkCode == 0x43 && GetAsyncKeyState(VK_CONTROL.0 as i32) < 0 {
            let now = now_ms();
            let last = LAST_C_TIME_MS.load(Ordering::Relaxed);

            if last != 0 && now - last < DOUBLE_PRESS_MS {
                // Double Ctrl+C within the window → fire translation
                LAST_C_TIME_MS.store(0, Ordering::Relaxed);
                if let Ok(guard) = UI_TX.lock() {
                    if let Some(tx) = guard.as_ref() {
                        let _ = tx.send(UiMsg::HotkeyFired);
                    }
                }
            } else {
                LAST_C_TIME_MS.store(now, Ordering::Relaxed);
            }
        }
    }

    // Always forward — never block the event
    CallNextHookEx(None, n_code, w_param, l_param)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ── Non-Windows stub ──────────────────────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
fn run_hotkey_loop(_ui_tx: mpsc::Sender<UiMsg>) {}
