use crate::provider::types::LightId;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Light not found: {0:?}")]
    NotFound(LightId),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Provider not configured: {0}")]
    NotConfigured(String),
    #[error("Discovery failed: {0}")]
    DiscoveryFailed(String),
    #[error("Set brightness failed: {0}")]
    SetBrightnessFailed(String),
    #[error("Failed to connect to PipeWire: {0}")]
    PipeWireConnection(String),
    #[error("PipeWire node not found: {0}")]
    NodeNotFound(String),
}
