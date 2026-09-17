//! Tray icon, menu and Win32 message loop.
//!
//! The PS original taught us: a tray loop without a message window dies.
//! tray-icon creates its own message window on Windows, but the creating
//! thread must pump messages — so the main thread runs a classic
//! `GetMessageW` loop and drains the crate's crossbeam receivers after
//! every message. Worker threads (connect/mirror) post `WM_APP` back to
//! this thread to wake it for tooltip updates.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, TranslateMessage, WM_APP,
};

use crate::config::Config;
use crate::{adb, log};

static EXIT: AtomicBool = AtomicBool::new(false);

const ID_CONNECT: &str = "connect";
const ID_MIRROR: &str = "mirror";
const ID_PAIR: &str = "pair";
const ID_SETTINGS: &str = "settings";
const ID_OPEN_LOG: &str = "open_log";
const ID_EXIT: &str = "exit";

/// Run the tray until the user picks Exit. Blocks the calling thread.
pub fn run(cfg: Config) -> anyhow::Result<()> {
    let icon = build_icon()?;

    let connect = MenuItem::with_id(ID_CONNECT, "Connect now", true, None);
    let mirror = MenuItem::with_id(ID_MIRROR, "Mirror", true, None);
    // dialogs run in a separate process (--pair / --settings)
    let pair = MenuItem::with_id(ID_PAIR, "Pair device...", true, None);
    let settings = MenuItem::with_id(ID_SETTINGS, "Settings...", true, None);
    let open_log = MenuItem::with_id(ID_OPEN_LOG, "Open log", true, None);
    let exit = MenuItem::with_id(ID_EXIT, "Exit", true, None);

    let menu = Menu::new();
    menu.append_items(&[
        &connect,
        &mirror,
        &PredefinedMenuItem::separator(),
        &pair,
        &settings,
        &PredefinedMenuItem::separator(),
        &open_log,
        &exit,
    ])?;

    let mut tray = TrayIconBuilder::new()
        .with_tooltip(format!(
            "droidbridge-rs — {}",
            if cfg.device_host.is_empty() {
                "no phone configured".to_string()
            } else {
                cfg.device_host.clone()
            }
        ))
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()?;

    // Tooltip updates flow from worker threads back to this one.
    let (tip_tx, tip_rx) = mpsc::channel::<String>();
    let main_thread_id = unsafe { GetCurrentThreadId() };

    log::write("tray started");
    // first-run convenience: nothing configured yet — open Settings
    if cfg.device_host.is_empty() {
        launch_dialog("--settings");
    }
    unsafe {
        let mut msg = MSG::default();
        'outer: loop {
            drain_events(&cfg, &mut tray, &tip_tx, &tip_rx, main_thread_id);
            if EXIT.load(Ordering::Relaxed) {
                break 'outer;
            }

            let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if r == 0 || r == -1 {
                break 'outer;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    drop(tray);
    log::write("tray exited");
    Ok(())
}

/// Process pending menu/tray events and apply queued tooltip updates.
fn drain_events(
    cfg: &Config,
    tray: &mut TrayIcon,
    tip_tx: &mpsc::Sender<String>,
    tip_rx: &mpsc::Receiver<String>,
    main_thread_id: u32,
) {
    while let Ok(ev) = MenuEvent::receiver().try_recv() {
        match ev.id().as_ref() {
            ID_CONNECT => spawn_action(cfg.clone(), tip_tx.clone(), main_thread_id, false),
            ID_MIRROR => spawn_action(cfg.clone(), tip_tx.clone(), main_thread_id, true),
            ID_PAIR => launch_dialog("--pair"),
            ID_SETTINGS => launch_dialog("--settings"),
            ID_OPEN_LOG => open_log(),
            ID_EXIT => EXIT.store(true, Ordering::Relaxed),
            _ => {}
        }
    }
    while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
        if let tray_icon::TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        } = ev
        {
            // double-click = Mirror (parity with the PS tray)
            spawn_action(cfg.clone(), tip_tx.clone(), main_thread_id, true);
        }
    }
    while let Ok(tip) = tip_rx.try_recv() {
        let _ = tray.set_tooltip(Some(&tip));
    }
}

fn spawn_action(cfg: Config, tip_tx: mpsc::Sender<String>, main_thread_id: u32, mirror: bool) {
    thread::spawn(move || {
        let status = match do_connect(&cfg, mirror) {
            Ok(Some(serial)) => format!("droidbridge-rs — connected: {serial}"),
            Ok(None) => "droidbridge-rs — could not connect".to_string(),
            Err(e) => format!("droidbridge-rs — {e}"),
        };
        let _ = tip_tx.send(status);
        // wake the message loop so the tooltip applies immediately
        unsafe {
            PostThreadMessageW(main_thread_id, WM_APP, 0, 0);
        }
    });
}

fn do_connect(cfg: &Config, mirror: bool) -> anyhow::Result<Option<String>> {
    // re-read the config so Settings changes apply without a tray restart
    let cfg = Config::load_or_create().unwrap_or_else(|_| cfg.clone());
    let Some(adb_path) = adb::resolve_adb(&cfg) else {
        anyhow::bail!("adb.exe not found - set adbPath in config");
    };
    let serial = adb::connect_phone(&adb_path, &cfg, true)?;
    if serial.is_some() && mirror {
        adb::launch_scrcpy_once(&cfg)?;
    }
    Ok(serial)
}

fn launch_dialog(flag: &str) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe).arg(flag).spawn();
    }
}

fn open_log() {
    if let Some(path) = log::log_path() {
        let _ = Command::new("notepad.exe").arg(&path).spawn();
    }
}

/// 32x32 icon drawn in code: teal disc with a white phone slab.
/// A real asset can replace this at release time.
fn build_icon() -> anyhow::Result<Icon> {
    const S: usize = 32;
    let mut rgba = vec![0u8; S * S * 4];
    let phone = |x: usize, y: usize| (12..20).contains(&x) && (8..24).contains(&y);
    for y in 0..S {
        for x in 0..S {
            let dx = (x as f32 + 0.5) - S as f32 / 2.0;
            let dy = (y as f32 + 0.5) - S as f32 / 2.0;
            if (dx * dx + dy * dy).sqrt() > 14.0 {
                continue;
            }
            let px = &mut rgba[(y * S + x) * 4..(y * S + x) * 4 + 4];
            if phone(x, y) {
                // white phone slab
                px.copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
            } else {
                // teal disc
                px.copy_from_slice(&[0x00, 0x89, 0x7b, 0xff]);
            }
        }
    }
    Ok(Icon::from_rgba(rgba, S as u32, S as u32)?)
}
