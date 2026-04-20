
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub bot: BotConfig,
    pub ai: AIConfig,
    pub db: DBConfig,
    pub channels: ChannelConfig,
    pub web: WebConfig,
}

#[derive(Debug, Deserialize)]
pub struct BotConfig {
    pub token: String,
    pub prefix: String,
}

#[derive(Debug, Deserialize)]
pub struct AIConfig {
    pub api_url: String,
    pub model: String,
    pub api_key: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

#[derive(Debug, Deserialize)]
pub struct DBConfig {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct TasksConfig {
    pub heartbeat: String,
    pub memory_cleanup: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChannelConfig {
    pub always_respond_channels: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WebConfig {
    pub listen: String,
    pub auth_user: Option<String>,
    pub auth_pass: Option<String>,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:3000".to_string(),
            auth_user: None,
            auth_pass: None,
        }
    }
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let path = Path::new(path);
        let toml_str = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&toml_str)?;
        Ok(config)
    }
}
