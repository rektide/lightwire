use std::collections::HashMap;
use super::error::ProviderError;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LightId(pub String);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brightness(pub f32);

impl Brightness {
    pub fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn as_f32(&self) -> f32 {
        self.0
    }

    pub fn as_u16(&self) -> u16 {
        (self.0 * 65535.0) as u16
    }

    pub fn as_percent(&self) -> u8 {
        (self.0 * 100.0) as u8
    }
}

impl Default for Brightness {
    fn default() -> Self {
        Self(0.0)
    }
}

#[derive(Clone, Debug)]
pub struct LightState {
    pub id: LightId,
    pub label: String,
    pub brightness: Brightness,
    pub power: bool,
}

impl LightState {
    pub fn new(id: LightId, label: String, brightness: Brightness, power: bool) -> Self {
        Self { id, label, brightness, power }
    }
}

pub trait Light: Send + Sync + std::fmt::Debug {
    fn id(&self) -> &LightId;
    fn label(&self) -> &str;
    fn provider_name(&self) -> &str;
    fn state(&self) -> &LightState;

    fn to_state(&self) -> LightState {
        self.state().clone()
    }

    fn metadata(&self) -> Option<&HashMap<String, String>> {
        None
    }
}

use async_trait::async_trait;

#[async_trait]
pub trait Provider: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;
    async fn discover(&self) -> Result<Vec<Box<dyn Light>>, ProviderError>;
    async fn get_state(&self, id: &LightId) -> Result<LightState, ProviderError>;
    async fn set_brightness(&self, id: &LightId, brightness: Brightness) -> Result<(), ProviderError>;

    async fn health_check(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_id_equality() {
        let id1 = LightId("test-id".to_string());
        let id2 = LightId("test-id".to_string());
        let id3 = LightId("other-id".to_string());

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_light_id_hash() {
        use std::collections::HashSet;

        let id1 = LightId("test-id".to_string());
        let id2 = LightId("test-id".to_string());
        let id3 = LightId("other-id".to_string());

        let mut set = HashSet::new();
        set.insert(id1.clone());
        assert!(set.contains(&id2));
        assert!(!set.contains(&id3));
    }

    #[test]
    fn test_brightness_new_clamps() {
        assert_eq!(Brightness::new(1.5).as_f32(), 1.0);
        assert_eq!(Brightness::new(-0.5).as_f32(), 0.0);
        assert_eq!(Brightness::new(0.5).as_f32(), 0.5);
    }

    #[test]
    fn test_brightness_conversions() {
        let b = Brightness::new(0.5);

        assert_eq!(b.as_f32(), 0.5);
        assert_eq!(b.as_u16(), 32767);
        assert_eq!(b.as_percent(), 50);
    }

    #[test]
    fn test_brightness_default() {
        let b = Brightness::default();
        assert_eq!(b.as_f32(), 0.0);
    }

    #[test]
    fn test_light_state_new() {
        let id = LightId("test-id".to_string());
        let state = LightState::new(
            id.clone(),
            "Test Light".to_string(),
            Brightness::new(0.75),
            true,
        );

        assert_eq!(state.id, id);
        assert_eq!(state.label, "Test Light");
        assert_eq!(state.brightness.as_f32(), 0.75);
        assert!(state.power);
    }
}
