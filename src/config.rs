use directories::ProjectDirs;
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub pipewire: PipewireConfig,
    #[serde(default)]
    pub curves: CurvesConfig,
    #[serde(default)]
    pub lifx: LifxConfig,
    #[serde(default)]
    pub lights: LightsConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pipewire: PipewireConfig::default(),
            curves: CurvesConfig::default(),
            lifx: LifxConfig::default(),
            lights: LightsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PipewireConfig {
    #[serde(default = "default_config_dir")]
    pub config_dir: Option<String>,
    #[serde(default = "default_node_prefix")]
    pub node_prefix: String,
}

fn default_config_dir() -> Option<String> {
    None
}

fn default_node_prefix() -> String {
    "lightwire".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CurvesConfig {
    #[serde(default = "default_curve")]
    pub default: String,
    #[serde(default)]
    pub custom: std::collections::HashMap<String, crate::curves::CurveConfig>,
}

impl Default for CurvesConfig {
    fn default() -> Self {
        Self {
            default: default_curve(),
            custom: std::collections::HashMap::new(),
        }
    }
}

fn default_curve() -> String {
    "perceptual".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LifxConfig {
    #[serde(default = "default_discovery_timeout")]
    pub discovery_timeout_ms: u64,
    #[serde(default = "default_broadcast_address")]
    pub broadcast_address: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for LifxConfig {
    fn default() -> Self {
        Self {
            discovery_timeout_ms: default_discovery_timeout(),
            broadcast_address: default_broadcast_address(),
            port: default_port(),
        }
    }
}

fn default_discovery_timeout() -> u64 {
    5000
}

fn default_broadcast_address() -> String {
    "255.255.255.255".to_string()
}

fn default_port() -> u16 {
    56700
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LightsConfig {
    #[serde(default)]
    pub lights: std::collections::HashMap<String, LightConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LightConfig {
    #[serde(default)]
    pub min_brightness: Option<f32>,
    #[serde(default)]
    pub max_brightness: Option<f32>,
    #[serde(default)]
    pub curve: Option<String>,
    #[serde(default)]
    pub mute_action: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

impl Config {
    pub fn load() -> Result<Self, figment::Error> {
        let dirs = ProjectDirs::from("com", "lightwire", "lightwire")
            .expect("Failed to determine project directories");

        let config_path = dirs.config_dir().join("config.toml");

        let figment = Figment::new()
            .merge(Toml::file(config_path))
            .merge(Env::prefixed("LIGHTWIRE_").split("_"));

        let config: Config = figment.extract()?;

        Ok(config)
    }

    pub fn load_from_path(path: PathBuf) -> Result<Self, figment::Error> {
        let figment = Figment::new().merge(Toml::file(path));

        let config: Config = figment.extract()?;

        Ok(config)
    }

    pub fn pipewire_config_dir(&self) -> PathBuf {
        if let Some(ref dir) = self.pipewire.config_dir {
            PathBuf::from(shellexpand::tilde(dir).into_owned())
        } else {
            let dirs = ProjectDirs::from("org", "freedesktop", "pipewire")
                .expect("Failed to determine PipeWire directories");
            dirs.config_dir().join("pipewire.conf.d").to_path_buf()
        }
    }
}
