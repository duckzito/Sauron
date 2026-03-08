# Sauron Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a macOS Rust CLI that captures screenshots every 3 minutes, summarizes them via Ollama, and generates daily activity logs emailed via Resend.

**Architecture:** Single async tokio binary with modules for capture, processing, storage, summarization, scheduling, config, and CLI. SQLite for structured data, filesystem for files.

**Tech Stack:** Rust, tokio, clap, rusqlite, reqwest, serde/toml, chrono, base64

---

### Task 1: Project Scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/config.rs`
- Create: `src/error.rs`

**Step 1: Initialize Cargo project**

Run: `cargo init --name sauron`
Expected: Creates `Cargo.toml` and `src/main.rs`

**Step 2: Add dependencies to Cargo.toml**

Replace `Cargo.toml` with:

```toml
[package]
name = "sauron"
version = "0.1.0"
edition = "2021"
description = "Screen activity logger with LLM-powered summaries"

[dependencies]
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
rusqlite = { version = "0.31", features = ["bundled"] }
reqwest = { version = "0.12", features = ["json", "multipart"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
chrono = { version = "0.4", features = ["serde"] }
base64 = "0.22"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
dirs = "5"
anyhow = "1"
thiserror = "2"
```

**Step 3: Create error module**

Create `src/error.rs`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SauronError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Screenshot capture failed: {0}")]
    Capture(String),

    #[error("Ollama error: {0}")]
    Ollama(String),

    #[error("Email error: {0}")]
    Email(String),
}

pub type Result<T> = std::result::Result<T, SauronError>;
```

**Step 4: Create config module**

Create `src/config.rs`:

```rust
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
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

impl Default for EmailConfig {
    fn default() -> Self {
        Self { resend_api_key: String::new(), from: String::new(), to: String::new() }
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

    /// Expand ~ to home directory in a path string
    pub fn expand_path(path: &str) -> PathBuf {
        if path.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(&path[2..]);
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

impl Default for Config {
    fn default() -> Self {
        Self {
            capture: CaptureConfig::default(),
            ollama: OllamaConfig::default(),
            summary: SummaryConfig::default(),
            email: EmailConfig::default(),
            database: DatabaseConfig::default(),
        }
    }
}
```

**Step 5: Create minimal main.rs**

```rust
mod config;
mod error;

fn main() {
    println!("Sauron - The All-Seeing Eye");
}
```

**Step 6: Verify it compiles**

Run: `cargo build`
Expected: Successful compilation

**Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "feat: project scaffolding with config and error modules"
```

---

### Task 2: CLI Module

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs`

**Step 1: Create CLI module**

Create `src/cli.rs`:

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sauron", about = "Screen activity logger with LLM-powered summaries")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the Sauron daemon
    Start,
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status,
    /// Install launchd service for auto-start
    Install,
    /// Uninstall launchd service
    Uninstall,
    /// Manually trigger daily summary
    Summary {
        /// Date to summarize (YYYY-MM-DD), defaults to today
        #[arg(long)]
        date: Option<String>,
    },
    /// Print current configuration
    Config,
}
```

**Step 2: Update main.rs to use CLI**

```rust
mod cli;
mod config;
mod error;

use clap::Parser;
use cli::{Cli, Commands};
use config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sauron=info".parse()?)
        )
        .init();

    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Commands::Start => {
            tracing::info!("Starting Sauron daemon...");
            // TODO: implement daemon
        }
        Commands::Stop => {
            tracing::info!("Stopping Sauron daemon...");
            // TODO: implement stop
        }
        Commands::Status => {
            tracing::info!("Checking status...");
            // TODO: implement status
        }
        Commands::Install => {
            tracing::info!("Installing launchd service...");
            // TODO: implement install
        }
        Commands::Uninstall => {
            tracing::info!("Uninstalling launchd service...");
            // TODO: implement uninstall
        }
        Commands::Summary { date } => {
            let date_str = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
            tracing::info!("Generating summary for {}...", date_str);
            // TODO: implement summary
        }
        Commands::Config => {
            println!("{:#?}", config);
        }
    }

    Ok(())
}
```

**Step 3: Verify it compiles and runs**

Run: `cargo run -- config`
Expected: Prints default config struct

Run: `cargo run -- --help`
Expected: Shows help with all subcommands

**Step 4: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: CLI module with subcommands"
```

---

### Task 3: Database Module

**Files:**
- Create: `src/db.rs`

**Step 1: Create database module**

Create `src/db.rs`:

```rust
use rusqlite::Connection;
use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS screenshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                captured_at TEXT NOT NULL,
                file_path TEXT NOT NULL,
                summary TEXT,
                model_used TEXT,
                processing_method TEXT,
                processed_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS daily_summaries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                summary_date TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                file_path TEXT NOT NULL,
                screenshot_count INTEGER,
                email_sent_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );"
        )?;
        Ok(())
    }

    pub fn insert_screenshot(&self, captured_at: &str, file_path: &str) -> anyhow::Result<i64> {
        self.conn.execute(
            "INSERT INTO screenshots (captured_at, file_path) VALUES (?1, ?2)",
            [captured_at, file_path],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_screenshot_summary(
        &self,
        id: i64,
        summary: &str,
        model_used: &str,
        processing_method: &str,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE screenshots SET summary = ?1, model_used = ?2, processing_method = ?3, processed_at = datetime('now') WHERE id = ?4",
            rusqlite::params![summary, model_used, processing_method, id],
        )?;
        Ok(())
    }

    pub fn get_day_summaries(&self, date: &str) -> anyhow::Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT captured_at, summary FROM screenshots WHERE captured_at LIKE ?1 AND summary IS NOT NULL ORDER BY captured_at"
        )?;
        let pattern = format!("{}%", date);
        let rows = stmt.query_map([pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn get_day_screenshot_count(&self, date: &str) -> anyhow::Result<i64> {
        let pattern = format!("{}%", date);
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM screenshots WHERE captured_at LIKE ?1",
            [pattern],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn insert_daily_summary(
        &self,
        summary_date: &str,
        content: &str,
        file_path: &str,
        screenshot_count: i64,
    ) -> anyhow::Result<i64> {
        self.conn.execute(
            "INSERT OR REPLACE INTO daily_summaries (summary_date, content, file_path, screenshot_count) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![summary_date, content, file_path, screenshot_count],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_email_sent(&self, summary_date: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE daily_summaries SET email_sent_at = datetime('now') WHERE summary_date = ?1",
            [summary_date],
        )?;
        Ok(())
    }

    pub fn get_last_screenshot(&self) -> anyhow::Result<Option<(String, String)>> {
        let result = self.conn.query_row(
            "SELECT captured_at, file_path FROM screenshots ORDER BY captured_at DESC LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
```

**Step 2: Add module to main.rs**

Add `mod db;` to the module declarations in `src/main.rs`.

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Successful compilation

**Step 4: Commit**

```bash
git add src/db.rs src/main.rs
git commit -m "feat: database module with SQLite schema and queries"
```

---

### Task 4: Screenshot Capture Module

**Files:**
- Create: `src/capture.rs`

**Step 1: Create capture module**

Create `src/capture.rs`:

```rust
use std::path::PathBuf;
use std::process::Command;
use chrono::Local;

pub struct Capturer {
    screenshot_dir: PathBuf,
}

impl Capturer {
    pub fn new(screenshot_dir: PathBuf) -> Self {
        Self { screenshot_dir }
    }

    /// Take a screenshot and return (datetime_string, file_path)
    pub fn take_screenshot(&self) -> anyhow::Result<(String, PathBuf)> {
        let now = Local::now();
        let date_dir = now.format("%Y-%m-%d").to_string();
        let filename = now.format("%H-%M-%S.png").to_string();
        let captured_at = now.to_rfc3339();

        let dir = self.screenshot_dir.join(&date_dir);
        std::fs::create_dir_all(&dir)?;

        let file_path = dir.join(&filename);

        let status = Command::new("screencapture")
            .args(["-x", file_path.to_str().unwrap()])
            .status()?;

        if !status.success() {
            anyhow::bail!("screencapture exited with status: {}", status);
        }

        tracing::info!("Screenshot saved: {}", file_path.display());
        Ok((captured_at, file_path))
    }
}
```

**Step 2: Add module to main.rs**

Add `mod capture;` to module declarations.

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Successful compilation

**Step 4: Commit**

```bash
git add src/capture.rs src/main.rs
git commit -m "feat: screenshot capture module using macOS screencapture"
```

---

### Task 5: Ollama Processor Module

**Files:**
- Create: `src/processor.rs`

**Step 1: Create processor module**

Create `src/processor.rs`:

```rust
use reqwest::Client;
use serde_json::json;
use std::path::Path;

pub struct Processor {
    client: Client,
    base_url: String,
    vision_model: String,
    text_model: String,
}

const SCREENSHOT_PROMPT: &str = "Describe what the user is doing on their screen. Be concise (2-3 sentences). Focus on: application in use, task being performed, key content visible.";

impl Processor {
    pub fn new(base_url: String, vision_model: String, text_model: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            vision_model,
            text_model,
        }
    }

    /// Process a screenshot: try vision model first, fall back to OCR + text model
    /// Returns (summary, model_used, processing_method)
    pub async fn process_screenshot(&self, image_path: &Path) -> anyhow::Result<(String, String, String)> {
        // Try vision model first
        match self.try_vision(image_path).await {
            Ok(summary) => {
                tracing::info!("Vision model succeeded");
                return Ok((summary, self.vision_model.clone(), "vision".into()));
            }
            Err(e) => {
                tracing::warn!("Vision model failed: {}, trying OCR fallback", e);
            }
        }

        // Fall back to OCR + text model
        let ocr_text = self.ocr_screenshot(image_path).await?;
        let summary = self.summarize_text(&ocr_text).await?;
        Ok((summary, self.text_model.clone(), "ocr_fallback".into()))
    }

    async fn try_vision(&self, image_path: &Path) -> anyhow::Result<String> {
        let image_bytes = tokio::fs::read(image_path).await?;
        let image_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &image_bytes,
        );

        let url = format!("{}/api/generate", self.base_url);
        let body = json!({
            "model": self.vision_model,
            "prompt": SCREENSHOT_PROMPT,
            "images": [image_b64],
            "stream": false,
        });

        let resp = self.client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama returned status: {}", resp.status());
        }

        let json: serde_json::Value = resp.json().await?;
        let response = json["response"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No response field in Ollama output"))?;

        Ok(response.trim().to_string())
    }

    async fn ocr_screenshot(&self, image_path: &Path) -> anyhow::Result<String> {
        // Try macOS native OCR via a swift script
        let output = tokio::process::Command::new("swift")
            .args(["-e", &format!(
                r#"
import Vision
import AppKit

let url = URL(fileURLWithPath: "{}")
guard let image = NSImage(contentsOf: url),
      let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {{
    print("ERROR: Could not load image")
    exit(1)
}}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
try handler.perform([request])

let text = (request.results ?? [])
    .compactMap {{ $0.topCandidates(1).first?.string }}
    .joined(separator: "\n")
print(text)
"#,
                image_path.display()
            )])
            .output()
            .await?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() && text != "ERROR: Could not load image" {
                return Ok(text);
            }
        }

        anyhow::bail!("OCR failed for {}", image_path.display())
    }

    async fn summarize_text(&self, text: &str) -> anyhow::Result<String> {
        let url = format!("{}/api/generate", self.base_url);
        let prompt = format!(
            "The following text was extracted from a user's screen via OCR. {}\n\nExtracted text:\n{}",
            SCREENSHOT_PROMPT, text
        );

        let body = json!({
            "model": self.text_model,
            "prompt": prompt,
            "stream": false,
        });

        let resp = self.client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama returned status: {}", resp.status());
        }

        let json: serde_json::Value = resp.json().await?;
        let response = json["response"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No response field in Ollama output"))?;

        Ok(response.trim().to_string())
    }
}
```

**Step 2: Add `use base64::Engine;` import note**

The `base64` crate v0.22 requires importing the `Engine` trait to use `.encode()`. This is handled in the code above with `base64::Engine::encode`.

**Step 3: Add module to main.rs**

Add `mod processor;` to module declarations.

**Step 4: Verify it compiles**

Run: `cargo build`
Expected: Successful compilation

**Step 5: Commit**

```bash
git add src/processor.rs src/main.rs
git commit -m "feat: Ollama processor with vision + OCR fallback"
```

---

### Task 6: Daily Summarizer Module

**Files:**
- Create: `src/summarizer.rs`

**Step 1: Create summarizer module**

Create `src/summarizer.rs`:

```rust
use reqwest::Client;
use serde_json::json;
use std::path::PathBuf;

pub struct Summarizer {
    client: Client,
    base_url: String,
    text_model: String,
    output_dir: PathBuf,
}

impl Summarizer {
    pub fn new(base_url: String, text_model: String, output_dir: PathBuf) -> Self {
        Self {
            client: Client::new(),
            base_url,
            text_model,
            output_dir,
        }
    }

    /// Generate a daily summary from individual screenshot summaries
    /// Returns (markdown_content, file_path)
    pub async fn generate_daily_summary(
        &self,
        date: &str,
        entries: &[(String, String)], // (captured_at, summary)
    ) -> anyhow::Result<(String, PathBuf)> {
        if entries.is_empty() {
            anyhow::bail!("No entries to summarize for {}", date);
        }

        let timeline = entries
            .iter()
            .map(|(time, summary)| {
                let time_short = time
                    .split('T')
                    .nth(1)
                    .and_then(|t| t.get(..5))
                    .unwrap_or(time);
                format!("[{}] {}", time_short, summary)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Based on these chronological screen activity summaries from {}, \
            create a concise daily activity log in markdown format. \
            Group related activities, note time ranges, highlight key accomplishments. \
            Use headers and bullet points.\n\n{}",
            date, timeline
        );

        let url = format!("{}/api/generate", self.base_url);
        let body = json!({
            "model": self.text_model,
            "prompt": prompt,
            "stream": false,
        });

        let resp = self.client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama returned status: {}", resp.status());
        }

        let json: serde_json::Value = resp.json().await?;
        let content = json["response"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No response field"))?
            .trim()
            .to_string();

        // Add header
        let markdown = format!("# Daily Activity Log — {}\n\n{}\n", date, content);

        // Save to filesystem
        std::fs::create_dir_all(&self.output_dir)?;
        let file_path = self.output_dir.join(format!("{}.md", date));
        std::fs::write(&file_path, &markdown)?;
        tracing::info!("Daily summary saved: {}", file_path.display());

        Ok((markdown, file_path))
    }
}
```

**Step 2: Add module to main.rs**

Add `mod summarizer;` to module declarations.

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Successful compilation

**Step 4: Commit**

```bash
git add src/summarizer.rs src/main.rs
git commit -m "feat: daily summarizer module with Ollama integration"
```

---

### Task 7: Email Module (Resend)

**Files:**
- Create: `src/email.rs`

**Step 1: Create email module**

Create `src/email.rs`:

```rust
use reqwest::Client;
use serde_json::json;

pub struct Mailer {
    client: Client,
    api_key: String,
    from: String,
    to: String,
}

impl Mailer {
    pub fn new(api_key: String, from: String, to: String) -> Option<Self> {
        if api_key.is_empty() || from.is_empty() || to.is_empty() {
            tracing::warn!("Email not configured — daily summaries will not be emailed");
            return None;
        }
        Some(Self {
            client: Client::new(),
            api_key,
            from,
            to,
        })
    }

    pub async fn send_daily_summary(&self, date: &str, markdown_content: &str) -> anyhow::Result<()> {
        let subject = format!("Sauron Daily Log — {}", date);

        // Convert markdown to simple HTML (basic conversion)
        let html_content = format!(
            "<html><body><pre style=\"font-family: monospace; white-space: pre-wrap;\">{}</pre></body></html>",
            html_escape(markdown_content)
        );

        let body = json!({
            "from": self.from,
            "to": [self.to],
            "subject": subject,
            "html": html_content,
        });

        let resp = self.client
            .post("https://api.resend.com/emails")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Resend API error ({}): {}", status, text);
        }

        tracing::info!("Daily summary email sent for {}", date);
        Ok(())
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
```

**Step 2: Add module to main.rs**

Add `mod email;` to module declarations.

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Successful compilation

**Step 4: Commit**

```bash
git add src/email.rs src/main.rs
git commit -m "feat: email module with Resend API integration"
```

---

### Task 8: Scheduler & Daemon Module

**Files:**
- Create: `src/daemon.rs`

**Step 1: Create daemon module**

Create `src/daemon.rs`:

```rust
use crate::capture::Capturer;
use crate::config::Config;
use crate::db::Database;
use crate::email::Mailer;
use crate::processor::Processor;
use crate::summarizer::Summarizer;

use chrono::Local;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

pub struct Daemon {
    config: Config,
}

impl Daemon {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let db = Arc::new(Mutex::new(Database::open(&self.config.db_path())?));
        let capturer = Capturer::new(self.config.screenshot_dir());
        let processor = Processor::new(
            self.config.ollama.base_url.clone(),
            self.config.ollama.vision_model.clone(),
            self.config.ollama.text_model.clone(),
        );
        let summarizer = Summarizer::new(
            self.config.ollama.base_url.clone(),
            self.config.ollama.text_model.clone(),
            self.config.output_dir(),
        );
        let mailer = Mailer::new(
            self.config.email.resend_api_key.clone(),
            self.config.email.from.clone(),
            self.config.email.to.clone(),
        );

        // Write PID file
        Self::write_pid()?;

        tracing::info!(
            "Sauron started — capturing every {} minutes",
            self.config.capture.interval_minutes
        );

        let capture_interval = Duration::from_secs(self.config.capture.interval_minutes * 60);

        // Spawn screenshot capture loop
        let db_capture = db.clone();
        let capture_handle = tokio::spawn(async move {
            let mut timer = interval(capture_interval);
            loop {
                timer.tick().await;
                match capturer.take_screenshot() {
                    Ok((captured_at, file_path)) => {
                        let file_path_str = file_path.to_string_lossy().to_string();
                        let id = {
                            let db = db_capture.lock().await;
                            match db.insert_screenshot(&captured_at, &file_path_str) {
                                Ok(id) => id,
                                Err(e) => {
                                    tracing::error!("DB insert failed: {}", e);
                                    continue;
                                }
                            }
                        };

                        // Process with Ollama
                        match processor.process_screenshot(&file_path).await {
                            Ok((summary, model, method)) => {
                                let db = db_capture.lock().await;
                                if let Err(e) = db.update_screenshot_summary(id, &summary, &model, &method) {
                                    tracing::error!("DB update failed: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Processing failed: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Screenshot capture failed: {}", e);
                    }
                }
            }
        });

        // Spawn daily summary loop
        let db_summary = db.clone();
        let daily_time = self.config.summary.daily_time.clone();
        let summary_handle = tokio::spawn(async move {
            loop {
                // Sleep until the target time
                let sleep_dur = Self::duration_until(&daily_time);
                tracing::info!("Next daily summary in {} seconds", sleep_dur.as_secs());
                tokio::time::sleep(sleep_dur).await;

                let today = Local::now().format("%Y-%m-%d").to_string();
                tracing::info!("Generating daily summary for {}", today);

                let entries = {
                    let db = db_summary.lock().await;
                    match db.get_day_summaries(&today) {
                        Ok(e) => e,
                        Err(e) => {
                            tracing::error!("Failed to get summaries: {}", e);
                            continue;
                        }
                    }
                };

                if entries.is_empty() {
                    tracing::info!("No summaries for today, skipping");
                    continue;
                }

                let screenshot_count = entries.len() as i64;

                match summarizer.generate_daily_summary(&today, &entries).await {
                    Ok((content, file_path)) => {
                        let file_path_str = file_path.to_string_lossy().to_string();
                        {
                            let db = db_summary.lock().await;
                            if let Err(e) = db.insert_daily_summary(&today, &content, &file_path_str, screenshot_count) {
                                tracing::error!("Failed to insert daily summary: {}", e);
                            }
                        }

                        // Send email
                        if let Some(ref mailer) = mailer {
                            match mailer.send_daily_summary(&today, &content).await {
                                Ok(_) => {
                                    let db = db_summary.lock().await;
                                    let _ = db.update_email_sent(&today);
                                }
                                Err(e) => {
                                    tracing::error!("Failed to send email: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to generate daily summary: {}", e);
                    }
                }
            }
        });

        // Wait for both tasks (runs forever)
        tokio::select! {
            _ = capture_handle => tracing::error!("Capture loop ended unexpectedly"),
            _ = summary_handle => tracing::error!("Summary loop ended unexpectedly"),
        }

        Ok(())
    }

    fn duration_until(time_str: &str) -> Duration {
        let parts: Vec<u32> = time_str.split(':').filter_map(|s| s.parse().ok()).collect();
        let (target_h, target_m) = (parts.get(0).copied().unwrap_or(23), parts.get(1).copied().unwrap_or(59));

        let now = Local::now();
        let today_target = now.date_naive()
            .and_hms_opt(target_h, target_m, 0)
            .unwrap();

        let target = if today_target > now.naive_local() {
            today_target
        } else {
            today_target + chrono::Duration::days(1)
        };

        let diff = target - now.naive_local();
        Duration::from_secs(diff.num_seconds().max(1) as u64)
    }

    fn write_pid() -> anyhow::Result<()> {
        let pid_path = Config::config_dir().join("sauron.pid");
        std::fs::create_dir_all(Config::config_dir())?;
        std::fs::write(&pid_path, std::process::id().to_string())?;
        Ok(())
    }

    pub fn read_pid() -> Option<u32> {
        let pid_path = Config::config_dir().join("sauron.pid");
        std::fs::read_to_string(pid_path).ok()?.trim().parse().ok()
    }

    pub fn remove_pid() {
        let pid_path = Config::config_dir().join("sauron.pid");
        let _ = std::fs::remove_file(pid_path);
    }
}
```

**Step 2: Add module to main.rs**

Add `mod daemon;` to module declarations.

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Successful compilation

**Step 4: Commit**

```bash
git add src/daemon.rs src/main.rs
git commit -m "feat: daemon module with capture and summary scheduling"
```

---

### Task 9: Wire Up CLI Commands

**Files:**
- Modify: `src/main.rs`
- Create: `src/launchd.rs`

**Step 1: Create launchd module**

Create `src/launchd.rs`:

```rust
use crate::config::Config;
use std::path::PathBuf;

const PLIST_LABEL: &str = "com.sauron.agent";

pub fn install() -> anyhow::Result<()> {
    let plist_path = plist_path();
    let binary_path = std::env::current_exe()?;

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>start</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}/sauron.log</string>
    <key>StandardErrorPath</key>
    <string>{}/sauron.err.log</string>
</dict>
</plist>"#,
        PLIST_LABEL,
        binary_path.display(),
        Config::config_dir().display(),
        Config::config_dir().display(),
    );

    std::fs::create_dir_all(plist_path.parent().unwrap())?;
    std::fs::write(&plist_path, plist_content)?;

    let status = std::process::Command::new("launchctl")
        .args(["load", plist_path.to_str().unwrap()])
        .status()?;

    if status.success() {
        tracing::info!("Launchd service installed and loaded");
    } else {
        anyhow::bail!("Failed to load launchd service");
    }

    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    let plist_path = plist_path();

    if plist_path.exists() {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", plist_path.to_str().unwrap()])
            .status();

        std::fs::remove_file(&plist_path)?;
        tracing::info!("Launchd service uninstalled");
    } else {
        tracing::info!("No launchd service found");
    }

    Ok(())
}

fn plist_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join("Library/LaunchAgents")
        .join(format!("{}.plist", PLIST_LABEL))
}
```

**Step 2: Update main.rs with full command wiring**

Replace `src/main.rs`:

```rust
mod capture;
mod cli;
mod config;
mod daemon;
mod db;
mod email;
mod error;
mod launchd;
mod processor;
mod summarizer;

use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use daemon::Daemon;
use db::Database;
use email::Mailer;
use summarizer::Summarizer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sauron=info".parse()?)
        )
        .init();

    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Commands::Start => {
            let daemon = Daemon::new(config);
            daemon.run().await?;
        }

        Commands::Stop => {
            if let Some(pid) = Daemon::read_pid() {
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
                Daemon::remove_pid();
                println!("Sauron stopped (PID {})", pid);
            } else {
                println!("Sauron is not running");
            }
        }

        Commands::Status => {
            if let Some(pid) = Daemon::read_pid() {
                // Check if process is actually running
                let running = unsafe { libc::kill(pid as i32, 0) == 0 };
                if running {
                    println!("Sauron is running (PID {})", pid);
                    let db = Database::open(&config.db_path())?;
                    if let Some((time, _path)) = db.get_last_screenshot()? {
                        println!("Last screenshot: {}", time);
                    }
                    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                    let count = db.get_day_screenshot_count(&today)?;
                    println!("Screenshots today: {}", count);
                } else {
                    Daemon::remove_pid();
                    println!("Sauron is not running (stale PID file cleaned)");
                }
            } else {
                println!("Sauron is not running");
            }
        }

        Commands::Install => {
            launchd::install()?;
        }

        Commands::Uninstall => {
            launchd::uninstall()?;
        }

        Commands::Summary { date } => {
            let date_str = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
            let db = Database::open(&config.db_path())?;
            let entries = db.get_day_summaries(&date_str)?;

            if entries.is_empty() {
                println!("No summaries found for {}", date_str);
                return Ok(());
            }

            let screenshot_count = entries.len() as i64;
            let summarizer = Summarizer::new(
                config.ollama.base_url.clone(),
                config.ollama.text_model.clone(),
                config.output_dir(),
            );

            let (content, file_path) = summarizer.generate_daily_summary(&date_str, &entries).await?;
            let file_path_str = file_path.to_string_lossy().to_string();
            db.insert_daily_summary(&date_str, &content, &file_path_str, screenshot_count)?;

            // Try to send email
            if let Some(mailer) = Mailer::new(
                config.email.resend_api_key.clone(),
                config.email.from.clone(),
                config.email.to.clone(),
            ) {
                match mailer.send_daily_summary(&date_str, &content).await {
                    Ok(_) => {
                        db.update_email_sent(&date_str)?;
                        println!("Summary emailed for {}", date_str);
                    }
                    Err(e) => {
                        println!("Summary saved but email failed: {}", e);
                    }
                }
            }

            println!("Summary saved to {}", file_path.display());
        }

        Commands::Config => {
            println!("Config path: {}", Config::config_path().display());
            println!("{:#?}", config);
        }
    }

    Ok(())
}
```

**Step 3: Add `libc` dependency to Cargo.toml**

Add to `[dependencies]`: `libc = "0.2"`

**Step 4: Verify it compiles**

Run: `cargo build`
Expected: Successful compilation

**Step 5: Test CLI**

Run: `cargo run -- --help`
Expected: Shows all subcommands

Run: `cargo run -- config`
Expected: Prints config path and default config

**Step 6: Commit**

```bash
git add src/ Cargo.toml Cargo.lock
git commit -m "feat: wire up all CLI commands with launchd support"
```

---

### Task 10: Integration Testing

**Files:**
- Modify: `src/main.rs` (if needed)

**Step 1: Build release binary**

Run: `cargo build --release`
Expected: Successful compilation

**Step 2: Test the full capture pipeline manually**

Run: `cargo run -- start`
Expected: Starts daemon, takes first screenshot after interval, logs activity.
Press Ctrl+C to stop.

**Step 3: Test status command**

In another terminal: `cargo run -- status`
Expected: Shows running state and screenshot count.

**Step 4: Test manual summary**

Run: `cargo run -- summary --date $(date +%Y-%m-%d)`
Expected: Generates summary markdown or reports no data.

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: integration test fixes"
```

---

### Task 11: Polish & Documentation

**Files:**
- Create: `.gitignore`

**Step 1: Create .gitignore**

```
/target
*.db
*.pid
*.log
```

**Step 2: Final build check**

Run: `cargo clippy -- -W warnings`
Run: `cargo build --release`
Expected: No warnings, successful build.

**Step 3: Commit**

```bash
git add .gitignore
git commit -m "chore: add gitignore and final polish"
```
