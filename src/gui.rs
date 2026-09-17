//! Pair and Settings dialogs (egui/eframe), each in its own process:
//! the tray launches `droidbridge_rs --pair` / `--settings` so the Win32
//! message loop and the winit loop never fight over the main thread.
//! Dialogs write the shared config directly; the tray re-reads it before
//! every action, so changes apply without a restart.

use eframe::egui;

use crate::adb;
use crate::config::Config;

pub fn run_pair() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([330.0, 240.0])
            .with_title("DroidBridge — Pair device"),
        ..Default::default()
    };
    eframe::run_native(
        "droidbridge-pair",
        options,
        Box::new(|_cc| Ok(Box::new(PairApp::default()))),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}

pub fn run_settings() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([430.0, 360.0])
            .with_title("DroidBridge — Settings"),
        ..Default::default()
    };
    eframe::run_native(
        "droidbridge-settings",
        options,
        Box::new(|_cc| {
            let cfg = Config::load_or_create().unwrap_or_default();
            Ok(Box::new(SettingsApp::from(cfg)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}

// ------------------------------------------------------------------ pair ---

#[derive(Default)]
struct PairApp {
    ip: String,
    port: String,
    code: String,
    busy: bool,
    result: String,
}

thread_local! {
    static PAIR_RESULT: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

impl eframe::App for PairApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Pair device");
            ui.add_space(6.0);
            ui.label("Phone: Settings -> Developer options -> Wireless debugging");
            ui.label("-> Pair device with pairing code, then enter the values:");
            ui.add_space(6.0);
            egui::Grid::new("pair").num_columns(2).show(ui, |ui| {
                ui.label("Phone IP:");
                ui.text_edit_singleline(&mut self.ip);
                ui.end_row();
                ui.label("Pairing port:");
                ui.text_edit_singleline(&mut self.port);
                ui.end_row();
                ui.label("6-digit code:");
                ui.text_edit_singleline(&mut self.code);
                ui.end_row();
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Pair"))
                    .clicked()
                {
                    self.busy = true;
                    self.result.clear();
                    let (ip, port, code) = (self.ip.clone(), self.port.clone(), self.code.clone());
                    let repaint_ctx = ctx.clone();
                    std::thread::spawn(move || {
                        let out = pair_blocking(&ip, &port, &code);
                        PAIR_RESULT.with(|r| *r.borrow_mut() = Some(out));
                        repaint_ctx.request_repaint();
                    });
                }
                if ui.button("Close").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            if self.busy {
                ui.spinner();
                if let Some(res) = PAIR_RESULT.with(|r| r.borrow_mut().take()) {
                    self.busy = false;
                    self.result = res;
                }
            }
            if !self.result.is_empty() {
                ui.add_space(6.0);
                ui.label(&self.result);
            }
        });
    }
}

fn pair_blocking(ip: &str, port: &str, code: &str) -> String {
    let (Ok(port), false) = (port.trim().parse::<u16>(), code.trim().is_empty()) else {
        return "Enter a valid port and code".into();
    };
    let cfg = Config::load_or_create().unwrap_or_default();
    let Some(adb_path) = adb::resolve_adb(&cfg) else {
        return "adb.exe not found - set adbPath in Settings".into();
    };
    match adb::pair(&adb_path, ip.trim(), port, code.trim()) {
        Ok(out) => {
            crate::log::write(&format!("paired with {ip}:{port}"));
            format!("Success: {out}")
        }
        Err(e) => format!("Failed: {e}"),
    }
}

// --------------------------------------------------------------- settings ---

struct SettingsApp {
    adb_path: String,
    scrcpy_path: String,
    device_host: String,
    device_bt_mac: String,
    port_min: String,
    port_max: String,
    wait_for_wifi_seconds: String,
    bt_trigger_enabled: bool,
    scrcpy_on_connect: bool,
    saved: String,
}

impl From<Config> for SettingsApp {
    fn from(c: Config) -> Self {
        Self {
            adb_path: c.adb_path,
            scrcpy_path: c.scrcpy_path,
            device_host: c.device_host,
            device_bt_mac: c.device_bt_mac,
            port_min: c.port_min.to_string(),
            port_max: c.port_max.to_string(),
            wait_for_wifi_seconds: c.wait_for_wifi_seconds.to_string(),
            bt_trigger_enabled: c.bt_trigger_enabled,
            scrcpy_on_connect: c.scrcpy_on_connect,
            saved: String::new(),
        }
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Settings");
            ui.add_space(6.0);
            egui::Grid::new("settings").num_columns(2).show(ui, |ui| {
                ui.label("adb.exe full path (empty = autodetect):");
                ui.text_edit_singleline(&mut self.adb_path);
                ui.end_row();
                ui.label("scrcpy.exe full path (empty = autodetect):");
                ui.text_edit_singleline(&mut self.scrcpy_path);
                ui.end_row();
                ui.label("Phone IP (Tailscale IP recommended):");
                ui.text_edit_singleline(&mut self.device_host);
                ui.end_row();
                ui.label("Phone BT MAC (empty = any BT device):");
                ui.text_edit_singleline(&mut self.device_bt_mac);
                ui.end_row();
                ui.label("Scan range (port min / max):");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.port_min).desired_width(70.0));
                    ui.add(egui::TextEdit::singleline(&mut self.port_max).desired_width(70.0));
                });
                ui.end_row();
                ui.label("Wi-Fi wait after BT connect (seconds):");
                ui.add(
                    egui::TextEdit::singleline(&mut self.wait_for_wifi_seconds).desired_width(70.0),
                );
                ui.end_row();
                ui.label("Bluetooth trigger task enabled:");
                ui.checkbox(&mut self.bt_trigger_enabled, "");
                ui.end_row();
                ui.label("Launch scrcpy on connect:");
                ui.checkbox(&mut self.scrcpy_on_connect, "");
                ui.end_row();
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.saved = match self.to_config().and_then(|c| c.save().map(|_| c)) {
                        Ok(_) => {
                            crate::log::write("settings saved");
                            "Saved".into()
                        }
                        Err(e) => format!("Error: {e}"),
                    };
                }
                if ui.button("Close").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if !self.saved.is_empty() {
                    ui.label(&self.saved);
                }
            });
        });
    }
}

impl SettingsApp {
    fn to_config(&self) -> anyhow::Result<Config> {
        Ok(Config {
            adb_path: self.adb_path.trim().to_string(),
            scrcpy_path: self.scrcpy_path.trim().to_string(),
            device_host: self.device_host.trim().to_string(),
            device_bt_mac: self.device_bt_mac.trim().to_string(),
            port_min: self.port_min.trim().parse()?,
            port_max: self.port_max.trim().parse()?,
            wait_for_wifi_seconds: self.wait_for_wifi_seconds.trim().parse()?,
            bt_trigger_enabled: self.bt_trigger_enabled,
            scrcpy_on_connect: self.scrcpy_on_connect,
        })
    }
}
