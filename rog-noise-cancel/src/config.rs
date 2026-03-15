use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseCancelConfig {
    pub mic_enabled: bool,
    pub speaker_enabled: bool,
    /// Suppression strength 0-100.
    pub suppression_level: u8,
}

impl Default for NoiseCancelConfig {
    fn default() -> Self {
        Self {
            mic_enabled: false,
            speaker_enabled: false,
            suppression_level: 75,
        }
    }
}
