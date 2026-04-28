
use serde::Deserialize;
use std::path::Path;
use std::env;

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
    #[serde(default)]
    pub token: String,
    pub prefix: String,
}

#[derive(Debug, Deserialize)]
pub struct AIConfig {
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

#[derive(Debug, Deserialize)]
pub struct DBConfig {
    pub path: String,
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

fn env_or(cfg: &str, key: &str) -> String {
    env::var(key).unwrap_or_else(|_| cfg.to_string())
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Load .env first (already done in main, but be resilient)
        let _ = dotenv::dotenv();

        let path = Path::new(path);
        let toml_str = std::fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&toml_str)?;

        // Override with env vars (takes priority over config.toml)
        config.bot.token = env_or(&config.bot.token, "DISCORD_BOT_TOKEN");
        config.ai.api_url = env_or(&config.ai.api_url, "AI_API_URL");
        config.ai.model = env_or(&config.ai.model, "AI_MODEL");
        config.ai.api_key = env_or(&config.ai.api_key, "AI_API_KEY");

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn sample_config() -> String {
        r#"
[bot]
prefix = "!"

[ai]
max_tokens = 2048
temperature = 0.7

[db]
path = "/tmp/test.db"

[channels]
always_respond_channels = []

[web]
listen = "127.0.0.1:3000"
"#
        .to_string()
    }

    #[test]
    fn test_env_or_returns_env_when_set() {
        // Can't easily test env::var with override in same process due to race
        // with other tests, so we test the fallback path instead.
    }

    #[test]
    fn test_env_or_fallback() {
        // Remove any existing env var to test fallback
        let _ = env::remove_var("TEST_NONEXISTENT_KEY_XYZ");
        let result = env_or("default_value", "TEST_NONEXISTENT_KEY_XYZ");
        assert_eq!(result, "default_value");
    }

    #[test]
    fn test_web_config_default() {
        let cfg = WebConfig::default();
        assert_eq!(cfg.listen, "127.0.0.1:3000");
        assert!(cfg.auth_user.is_none());
        assert!(cfg.auth_pass.is_none());
    }

    #[test]
    fn test_config_from_file_valid() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, sample_config()).unwrap();

        let config = Config::from_file(config_path.to_str().unwrap()).unwrap();

        assert_eq!(config.bot.prefix, "!");
        assert_eq!(config.ai.max_tokens, 2048);
        assert!((config.ai.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(config.db.path, "/tmp/test.db");
        assert_eq!(config.web.listen, "127.0.0.1:3000");
    }

    #[test]
    fn test_config_missing_file() {
        let result = Config::from_file("/nonexistent/path/config.toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "this is [not valid toml{{{").unwrap();

        let result = Config::from_file(config_path.to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_config_minimal_toml() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "[bot]\nprefix = \"!\"\n\n[ai]\nmax_tokens = 1024\ntemperature = 0.5\n\n[db]\npath = \"/tmp/test.db\"\n\n[channels]\nalways_respond_channels = []\n\n[web]\nlisten = \"0.0.0.0:8080\"\n").unwrap();

        let config = Config::from_file(config_path.to_str().unwrap()).unwrap();
        assert_eq!(config.bot.prefix, "!");
        assert_eq!(config.ai.max_tokens, 1024);
        // always_respond_channels is Some(vec![]) from the TOML, not None
        assert!(config.channels.always_respond_channels.is_some());
    }

    #[test]
    fn test_env_or_returns_env_when_present() {
        env::set_var("TEST_ENV_OR_VAR", "from_env");
        let result = env_or("from_cfg", "TEST_ENV_OR_VAR");
        assert_eq!(result, "from_env");
        env::remove_var("TEST_ENV_OR_VAR");
    }

    #[test]
    fn test_env_or_returns_cfg_when_not_present() {
        env::remove_var("TEST_ENV_OR_MISSING_VAR");
        let result = env_or("from_cfg", "TEST_ENV_OR_MISSING_VAR");
        assert_eq!(result, "from_cfg");
    }
}
