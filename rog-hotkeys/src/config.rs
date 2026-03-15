//! Hotkey configuration — stored at /etc/asusd/hotkeys.ron

use serde::{Deserialize, Serialize};

/// Action to perform when a hotkey is triggered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HotkeyAction {
    /// Cycle to the next thermal profile.
    ProfileNext,
    /// Cycle to the previous thermal profile.
    ProfilePrev,
    /// Cycle to the next Aura mode.
    AuraNext,
    /// Cycle to the previous Aura mode.
    AuraPrev,
    /// Cycle Aura brightness.
    AuraBrightnessNext,
    /// Open rog-control-center GUI.
    LaunchGui,
    /// Run a custom shell command.
    Command(String),
    /// Let the key pass through to the desktop environment.
    Passthrough,
}

/// Maps an evdev key code (u16) to an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key_code: u16,
    pub action: HotkeyAction,
}

/// Root config — serialised to /etc/asusd/hotkeys.ron
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeysConfig {
    pub bindings: Vec<KeyBinding>,
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self {
            bindings: vec![
                // Fn+F5 — KEY_PROG3 (0x1d2) — cycle thermal profiles
                KeyBinding { key_code: 466, action: HotkeyAction::ProfileNext },
                // Fn+F4 — KEY_KBDILLUMTOGGLE (0xe9) — cycle Aura modes
                KeyBinding { key_code: 233, action: HotkeyAction::AuraNext },
                // ROG button — KEY_PROG1 (0x1d0) — open GUI
                KeyBinding { key_code: 464, action: HotkeyAction::LaunchGui },
            ],
        }
    }
}
