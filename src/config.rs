use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub vibevoice: VibeVoiceConfig,
    pub voices: VoicesConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
}

fn default_listen_addr() -> String {
    "[::1]:50051".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct VibeVoiceConfig {
    #[serde(default = "default_api_url")]
    pub api_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_ddpm_steps")]
    pub ddpm_steps: u32,
    #[serde(default = "default_cfg_scale")]
    pub cfg_scale: f32,
}

fn default_api_url() -> String {
    "http://127.0.0.1:8000/v1".to_string()
}

fn default_model() -> String {
    "vibevoice/VibeVoice-7B".to_string()
}

fn default_ddpm_steps() -> u32 {
    20
}

fn default_cfg_scale() -> f32 {
    3.0
}

#[derive(Debug, Deserialize)]
pub struct VoicesConfig {
    #[serde(default)]
    pub default: Option<String>,
    #[serde(flatten)]
    pub voices: HashMap<String, VoiceEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VoiceEntry {
    pub path: String,
    pub display_name: Option<String>,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ConfigError::ReadError(e.to_string()))?;
        toml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))
    }

    pub fn get_voice(&self, voice_id: &str) -> Result<&VoiceEntry, ConfigError> {
        let id = if voice_id.is_empty() {
            self.voices
                .default
                .as_deref()
                .ok_or(ConfigError::NoDefaultVoice)?
        } else {
            voice_id
        };

        self.voices
            .voices
            .get(id)
            .ok_or_else(|| ConfigError::VoiceNotFound(id.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    ReadError(String),
    #[error("failed to parse config: {0}")]
    ParseError(String),
    #[error("voice not found: {0}")]
    VoiceNotFound(String),
    #[error("no default voice configured and voice_id is empty")]
    NoDefaultVoice,
}
