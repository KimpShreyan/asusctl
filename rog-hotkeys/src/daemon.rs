//! Hotkey daemon — monitors evdev devices for ASUS keys.

use crate::config::{HotkeyAction, HotkeysConfig};

/// Entry point for the hotkey daemon.
/// Monitors ASUS keyboard input devices and dispatches D-Bus calls.
pub async fn run(_config: HotkeysConfig) {
    // TODO: implement evdev device detection and event loop
    // 1. Enumerate /dev/input/event* via udev (match ASUS vendor 0x0B05)
    // 2. Open matched devices with evdev::Device
    // 3. Async event loop reading InputEvents via tokio
    // 4. Map key_code -> HotkeyAction -> D-Bus call
}
