use serde::{Deserialize, Serialize};

/// Action to apply when a scenario matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioAction {
    /// Thermal profile name to apply (e.g. "Performance").
    pub profile: Option<String>,
    /// Aura mode to apply.
    pub aura_mode: Option<String>,
}

/// A rule: when process matching `process_regex` is running, apply `action`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioRule {
    pub name: String,
    pub enabled: bool,
    /// Regex matched against running process names.
    pub process_regex: String,
    pub action: ScenarioAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScenariosConfig {
    pub rules: Vec<ScenarioRule>,
}
