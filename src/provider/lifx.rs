use super::types::{Light, LightState, LightId, Brightness, Provider};
use super::error::ProviderError;
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug)]
pub struct LifxLight {
    state: LightState,
}

impl LifxLight {
    pub fn new(label: String, brightness: Brightness, power: bool) -> Self {
        let id = LightId(format!("lifx:{}", label));
        Self {
            state: LightState::new(id, label, brightness, power),
        }
    }
}

impl Light for LifxLight {
    fn id(&self) -> &LightId {
        &self.state.id
    }

    fn label(&self) -> &str {
        &self.state.label
    }

    fn provider_name(&self) -> &str {
        "lifx"
    }

    fn state(&self) -> &LightState {
        &self.state
    }
}

#[derive(Debug)]
pub struct LifxProvider {
    discovery_timeout: Duration,
    broadcast_address: String,
    port: u16,
}

impl LifxProvider {
    pub fn new(discovery_timeout_ms: u64, broadcast_address: String, port: u16) -> Self {
        Self {
            discovery_timeout: Duration::from_millis(discovery_timeout_ms),
            broadcast_address,
            port,
        }
    }

    pub fn default_config() -> Self {
        Self {
            discovery_timeout: Duration::from_millis(5000),
            broadcast_address: "255.255.255.255".to_string(),
            port: 56700,
        }
    }
}

impl Default for LifxProvider {
    fn default() -> Self {
        Self::default_config()
    }
}

#[async_trait]
impl Provider for LifxProvider {
    fn name(&self) -> &'static str {
        "lifx"
    }

    async fn discover(&self) -> Result<Vec<Box<dyn Light>>, ProviderError> {
        tracing::info!("LIFX discovery not yet implemented - returning stub lights");

        Ok(vec![
            Box::new(LifxLight::new("Stub Light 1".to_string(), Brightness::new(0.75), true)),
            Box::new(LifxLight::new("Stub Light 2".to_string(), Brightness::new(0.5), true)),
        ])
    }

    async fn get_state(&self, id: &LightId) -> Result<LightState, ProviderError> {
        Ok(LightState::new(
            id.clone(),
            "LIFX Light".to_string(),
            Brightness::new(0.5),
            true,
        ))
    }

    async fn set_brightness(&self, _id: &LightId, _brightness: Brightness) -> Result<(), ProviderError> {
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}
