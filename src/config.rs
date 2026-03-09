use chrono::Timelike;
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
    #[serde(default = "default_active_start")]
    pub active_hours_start: String,
    #[serde(default = "default_active_end")]
    pub active_hours_end: String,
    /// Optional list of display indices to capture. None = capture all.
    #[serde(default)]
    pub monitors: Option<Vec<usize>>,
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

#[derive(Default, Deserialize, Clone)]
pub struct EmailConfig {
    #[serde(default)]
    pub resend_api_key: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
}

impl std::fmt::Debug for EmailConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let masked_key = if self.resend_api_key.is_empty() {
            "not set".to_string()
        } else if self.resend_api_key.len() >= 4 {
            format!("***{}", &self.resend_api_key[self.resend_api_key.len() - 4..])
        } else {
            "***".to_string()
        };

        f.debug_struct("EmailConfig")
            .field("resend_api_key", &masked_key)
            .field("from", &self.from)
            .field("to", &self.to)
            .finish()
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

fn default_interval() -> u64 { 3 }
fn default_screenshot_dir() -> String { "~/sauron/screenshots".into() }
fn default_active_start() -> String { "09:00".into() }
fn default_active_end() -> String { "18:00".into() }
fn default_ollama_url() -> String { "http://localhost:11434".into() }
fn default_vision_model() -> String { "llava".into() }
fn default_text_model() -> String { "llama3.2".into() }
fn default_daily_time() -> String { "23:59".into() }
fn default_output_dir() -> String { "~/sauron/daily-logs".into() }
fn default_db_path() -> String { "~/sauron/sauron.db".into() }

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            interval_minutes: default_interval(),
            screenshot_dir: default_screenshot_dir(),
            active_hours_start: default_active_start(),
            active_hours_end: default_active_end(),
            monitors: None,
        }
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

    pub fn pause_file() -> PathBuf {
        Self::config_dir().join("sauron.paused")
    }

    pub fn is_paused() -> bool {
        Self::pause_file().exists()
    }

    pub fn is_within_active_hours(&self) -> bool {
        let now = chrono::Local::now();
        let current_minutes = now.hour() as u32 * 60 + now.minute() as u32;

        let start = Self::parse_time_minutes(&self.capture.active_hours_start).unwrap_or(0);
        let end = Self::parse_time_minutes(&self.capture.active_hours_end).unwrap_or(24 * 60);

        current_minutes >= start && current_minutes < end
    }

    fn parse_time_minutes(time_str: &str) -> Option<u32> {
        let parts: Vec<&str> = time_str.split(':').collect();
        let h: u32 = parts.first()?.parse().ok()?;
        let m: u32 = parts.get(1)?.parse().ok()?;
        Some(h * 60 + m)
    }
}

