//! PipeWire + RNNoise noise cancellation engine.

/// Noise cancellation engine.
pub struct NoiseCancelEngine;

impl NoiseCancelEngine {
    pub fn new() -> Self {
        Self
    }

    /// Start the noise cancellation filter chain in PipeWire.
    pub async fn start(&self) {
        // TODO: implement PipeWire filter chain with RNNoise
    }

    /// Stop the filter chain.
    pub async fn stop(&self) {
        // TODO: implement
    }
}
