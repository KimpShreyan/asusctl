//! Hotkey daemon — monitors evdev devices for ASUS keys.

use std::path::PathBuf;
use evdev::{Device, InputEventKind, Key};
use tokio::sync::mpsc;
use log::{info, warn, error};
use crate::config::{HotkeyAction, HotkeysConfig};
use crate::keys;

/// Find ASUS keyboard input devices via scanning /dev/input/
fn find_asus_devices() -> Vec<PathBuf> {
    let mut devices = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.starts_with("event") {
                    continue;
                }
                if let Ok(dev) = Device::open(&path) {
                    let dev_name = dev.name().unwrap_or("");
                    // Match ASUS keyboard devices
                    if dev_name.contains("Asus") || dev_name.contains("ASUS")
                        || dev_name.contains("asus")
                    {
                        info!("Found ASUS input device: {} at {}", dev_name, path.display());
                        devices.push(path);
                    }
                }
            }
        }
    }
    devices
}

/// Dispatch a hotkey action via D-Bus
async fn dispatch_action(action: &HotkeyAction) {
    match action {
        HotkeyAction::ProfileNext => {
            info!("Hotkey: cycling to next thermal profile");
            // TODO: call xyz.ljones.Platform.NextProfile via zbus
        }
        HotkeyAction::ProfilePrev => {
            info!("Hotkey: cycling to previous thermal profile");
        }
        HotkeyAction::AuraNext => {
            info!("Hotkey: cycling to next Aura mode");
            // TODO: call xyz.ljones.Aura.NextMode via zbus
        }
        HotkeyAction::AuraPrev => {
            info!("Hotkey: cycling to previous Aura mode");
        }
        HotkeyAction::AuraBrightnessNext => {
            info!("Hotkey: cycling Aura brightness");
        }
        HotkeyAction::LaunchGui => {
            info!("Hotkey: launching rog-control-center");
            let _ = tokio::process::Command::new("rog-control-center")
                .spawn();
        }
        HotkeyAction::Command(cmd) => {
            info!("Hotkey: running custom command: {}", cmd);
            let _ = tokio::process::Command::new("sh")
                .args(["-c", cmd])
                .spawn();
        }
        HotkeyAction::Passthrough => {}
    }
}

/// Main hotkey daemon event loop
pub async fn run(config: HotkeysConfig) {
    let devices = find_asus_devices();
    if devices.is_empty() {
        warn!("No ASUS input devices found. Hotkey daemon idle.");
        // Keep running — devices may appear later via hotplug
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let d = find_asus_devices();
            if !d.is_empty() {
                info!("ASUS input device appeared, restarting hotkey listener");
                return Box::pin(run(config)).await;
            }
        }
    }

    let (tx, mut rx) = mpsc::channel::<u16>(64);

    // Spawn reader threads for each device
    for path in devices {
        let tx = tx.clone();
        tokio::task::spawn_blocking(move || {
            let mut dev = match Device::open(&path) {
                Ok(d) => d,
                Err(e) => {
                    error!("Failed to open {}: {}", path.display(), e);
                    return;
                }
            };
            loop {
                match dev.fetch_events() {
                    Ok(events) => {
                        for ev in events {
                            if let InputEventKind::Key(key) = ev.kind() {
                                if ev.value() == 1 {
                                    // Key press (not release or repeat)
                                    let _ = tx.blocking_send(key.0);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading events from {}: {}", path.display(), e);
                        break;
                    }
                }
            }
        });
    }
    drop(tx); // Drop our copy so rx closes when all senders are gone

    // Dispatch loop
    while let Some(key_code) = rx.recv().await {
        if let Some(binding) = config.bindings.iter().find(|b| b.key_code == key_code) {
            dispatch_action(&binding.action).await;
        }
    }
    warn!("All input device readers exited. Hotkey daemon stopping.");
}
