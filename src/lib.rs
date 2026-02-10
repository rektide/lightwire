pub mod provider;
pub mod curves;
pub mod pipewire;
pub mod config;

pub use provider::{LightId, Brightness, LightState, Light, Provider, ProviderRegistry, ProviderError};
pub use curves::{Curve, CurveConfig, LinearCurve, LogarithmicCurve, GammaCurve, PerceptualCurve};
pub use pipewire::{DropinConfig, Volume, VolumeController, VolumeMonitor, VolumeEvent};
pub use config::{Config, PipewireConfig, CurvesConfig, LifxConfig, LightsConfig, LightConfig};
