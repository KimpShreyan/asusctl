use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A single key event in a macro recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroEvent {
    /// evdev key code
    pub key_code: u16,
    /// true = press, false = release
    pub pressed: bool,
    /// delay since previous event
    pub delay: Duration,
}

/// A recorded macro sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub events: Vec<MacroEvent>,
}

/// Macro store — all macros saved in ~/.config/rog/macros.ron
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MacroStore {
    pub macros: Vec<Macro>,
}

impl MacroStore {
    /// Find a macro by name
    pub fn find(&self, name: &str) -> Option<&Macro> {
        self.macros.iter().find(|m| m.name == name)
    }

    /// Add or replace a macro
    pub fn upsert(&mut self, mac: Macro) {
        if let Some(existing) = self.macros.iter_mut().find(|m| m.name == mac.name) {
            *existing = mac;
        } else {
            self.macros.push(mac);
        }
    }

    /// Delete a macro by name
    pub fn delete(&mut self, name: &str) -> bool {
        let len = self.macros.len();
        self.macros.retain(|m| m.name != name);
        self.macros.len() < len
    }

    /// List all macro names
    pub fn list_names(&self) -> Vec<&str> {
        self.macros.iter().map(|m| m.name.as_str()).collect()
    }
}
