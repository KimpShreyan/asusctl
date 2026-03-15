use serde::{Deserialize, Serialize};

/// Configurable action for the ROG button.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RogButtonAction {
    /// Open the ROG Control Center GUI.
    LaunchGui,
    /// Cycle to next thermal profile.
    CycleProfile,
    /// Toggle noise cancellation.
    ToggleNoiseCancel,
    /// Run a custom command.
    Command(String),
}

impl Default for RogButtonAction {
    fn default() -> Self {
        Self::LaunchGui
    }
}

/// Configuration for the ROG button.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RogButtonConfig {
    pub action: RogButtonAction,
}

impl Default for RogButtonConfig {
    fn default() -> Self {
        Self {
            action: RogButtonAction::LaunchGui,
        }
    }
}
