use std::collections::HashMap;
use super::types::{Light, LightId, Brightness, LightState, Provider};
use super::error::ProviderError as Error;

#[derive(Debug)]
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: HashMap::new() }
    }

    pub fn register(&mut self, provider: Box<dyn Provider>) {
        let name = provider.name().to_string();
        if self.providers.contains_key(&name) {
            tracing::warn!("Provider '{}' already registered, replacing", name);
        }
        self.providers.insert(name, provider);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Provider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    pub async fn discover_all(&self) -> Result<Vec<Box<dyn Light>>, Error> {
        let mut all_lights = Vec::new();
        for (name, provider) in &self.providers {
            tracing::info!("Discovering lights from provider: {}", name);
            match provider.discover().await {
                Ok(lights) => {
                    tracing::info!("Found {} lights from {}", lights.len(), name);
                    all_lights.extend(lights);
                }
                Err(e) => {
                    tracing::error!("Failed to discover from {}: {}", name, e);
                }
            }
        }
        Ok(all_lights)
    }

    pub async fn get_state(&self, provider_name: &str, id: &LightId) -> Result<LightState, Error> {
        match self.get(provider_name) {
            Some(provider) => provider.get_state(id).await,
            None => Err(Error::NotConfigured(format!("Provider '{}' not found", provider_name))),
        }
    }

    pub async fn set_brightness(&self, provider_name: &str, id: &LightId, brightness: Brightness) -> Result<(), Error> {
        match self.get(provider_name) {
            Some(provider) => provider.set_brightness(id, brightness).await,
            None => Err(Error::NotConfigured(format!("Provider '{}' not found", provider_name))),
        }
    }

    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    pub fn count(&self) -> usize {
        self.providers.len()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::types::{Light, LightState, Brightness, LightId};
    use crate::provider::error::ProviderError;
    use async_trait::async_trait;

    #[derive(Debug)]
    struct MockLight {
        state: LightState,
    }

    impl MockLight {
        fn new(id: &str, label: &str, brightness: f32) -> Self {
            Self {
                state: LightState::new(
                    LightId(id.to_string()),
                    label.to_string(),
                    Brightness::new(brightness),
                    true,
                ),
            }
        }
    }

    impl Light for MockLight {
        fn id(&self) -> &LightId {
            &self.state.id
        }

        fn label(&self) -> &str {
            &self.state.label
        }

        fn provider_name(&self) -> &str {
            "mock"
        }

        fn state(&self) -> &LightState {
            &self.state
        }
    }

    #[derive(Debug)]
    struct MockProvider {
        name: &'static str,
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn name(&self) -> &'static str {
            self.name
        }

        async fn discover(&self) -> Result<Vec<Box<dyn Light>>, ProviderError> {
            Ok(vec![
                Box::new(MockLight::new("id1", "Light 1", 0.5)),
                Box::new(MockLight::new("id2", "Light 2", 0.75)),
            ])
        }

        async fn get_state(&self, _id: &LightId) -> Result<LightState, ProviderError> {
            Ok(LightState::new(
                LightId("test".to_string()),
                "Test".to_string(),
                Brightness::new(0.5),
                true,
            ))
        }

        async fn set_brightness(&self, _id: &LightId, _brightness: Brightness) -> Result<(), ProviderError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_registry_new() {
        let registry = ProviderRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.count(), 0);
    }

    #[tokio::test]
    async fn test_registry_register() {
        let mut registry = ProviderRegistry::new();
        let provider = Box::new(MockProvider { name: "test" });

        registry.register(provider);
        assert_eq!(registry.count(), 1);
        assert!(registry.get("test").is_some());
    }

    #[tokio::test]
    async fn test_registry_register_replace() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MockProvider { name: "test" }));
        registry.register(Box::new(MockProvider { name: "test" }));

        assert_eq!(registry.count(), 1);
    }

    #[tokio::test]
    async fn test_registry_provider_names() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MockProvider { name: "lifx" }));
        registry.register(Box::new(MockProvider { name: "hue" }));

        let names = registry.provider_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"lifx"));
        assert!(names.contains(&"hue"));
    }

    #[tokio::test]
    async fn test_registry_discover_all() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MockProvider { name: "lifx" }));
        registry.register(Box::new(MockProvider { name: "hue" }));

        let lights = registry.discover_all().await.unwrap();
        assert_eq!(lights.len(), 4); // 2 per provider
    }

    #[tokio::test]
    async fn test_registry_get_state() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MockProvider { name: "test" }));

        let result = registry.get_state("test", &LightId("any".to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_registry_get_state_not_found() {
        let registry = ProviderRegistry::new();

        let result = registry.get_state("missing", &LightId("any".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_registry_set_brightness() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(MockProvider { name: "test" }));

        let result = registry.set_brightness("test", &LightId("any".to_string()), Brightness::new(0.5)).await;
        assert!(result.is_ok());
    }
}
