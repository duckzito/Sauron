use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub capture: CaptureConfig,
    #[serde(default)]
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub summary: SummaryConfig,
    #[serde(default)]
    pub email: EmailConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CaptureConfig {
    #[serde(default = "default_interval")]
    pub interval_minutes: u64,
    #[serde(default = "default_screenshot_dir")]
    pub screenshot_dir: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_url")]
    pub base_url: String,
    #[serde(default = "default_vision_model")]
    pub vision_model: String,
    #[serde(default = "default_text_model")]
    pub text_model: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SummaryConfig {
    #[serde(default = "default_daily_time")]
    pub daily_time: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct EmailConfig {
    #[serde(default)]
    pub resend_api_key: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

fn default_interval() -> u64 { 3 }
fn default_screenshot_dir() -> String { "~/sauron/screenshots".into() }
fn default_ollama_url() -> String { "http://localhost:11434".into() }
fn default_vision_model() -> String { "llava".into() }
fn default_text_model() -> String { "llama3.2".into() }
fn default_daily_time() -> String { "23:59".into() }
fn default_output_dir() -> String { "~/sauron/daily-logs".into() }
fn default_db_path() -> String { "~/sauron/sauron.db".into() }

impl Default for CaptureConfig {
    fn default() -> Self {
        Self { interval_minutes: default_interval(), screenshot_dir: default_screenshot_dir() }
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self { base_url: default_ollama_url(), vision_model: default_vision_model(), text_model: default_text_model() }
    }
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self { daily_time: default_daily_time(), output_dir: default_output_dir() }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self { path: default_db_path() }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::config_path();
        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("sauron")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn expand_path(path: &str) -> PathBuf {
        if let Some(stripped) = path.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(stripped);
            }
        }
        PathBuf::from(path)
    }

    pub fn screenshot_dir(&self) -> PathBuf {
        Self::expand_path(&self.capture.screenshot_dir)
    }

    pub fn output_dir(&self) -> PathBuf {
        Self::expand_path(&self.summary.output_dir)
    }

    pub fn db_path(&self) -> PathBuf {
        Self::expand_path(&self.database.path)
    }
}

