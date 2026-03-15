use crate::config::ScenariosConfig;

pub struct ScenarioMonitor {
    config: ScenariosConfig,
}

impl ScenarioMonitor {
    pub fn new(config: ScenariosConfig) -> Self {
        Self { config }
    }

    /// Start the monitoring loop — polls /proc via procfs every 5s.
    pub async fn run(&self) {
        // TODO: poll /proc via procfs every 5s, match running processes
        // against ScenarioRule patterns, call D-Bus to apply actions
        let _ = &self.config;
    }
}
