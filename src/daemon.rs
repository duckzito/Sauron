use crate::capture::Capturer;
use crate::config::Config;
use crate::db::Database;
use crate::email::Mailer;
use crate::processor::Processor;
use crate::summarizer::Summarizer;

use chrono::{Local, Timelike};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval_at, Duration, Instant};

/// Check whether the given PID belongs to a sauron process by inspecting
/// the process command name via `ps`. Returns `true` only when `ps`
/// reports a command that contains "sauron".
pub fn is_sauron_process(pid: u32) -> bool {
    std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .map(|output| {
            let comm = String::from_utf8_lossy(&output.stdout);
            comm.to_lowercase().contains("sauron")
        })
        .unwrap_or(false)
}

extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

pub struct Daemon {
    config: Config,
}

impl Daemon {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        // Single-instance guard: abort if another sauron daemon is already running
        if let Some(existing_pid) = Self::read_pid() {
            if existing_pid != std::process::id() && is_sauron_process(existing_pid) {
                anyhow::bail!(
                    "Another sauron instance is already running (PID {}). \
                     Stop it first with `sauron stop`.",
                    existing_pid
                );
            }
        }

        // Write PID file immediately after guard to minimize race window
        Self::write_pid()?;

        self.check_screen_permission()?;

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

        tracing::info!(
            "Sauron started — capturing every {} minutes",
            self.config.capture.interval_minutes
        );

        let capture_interval = Duration::from_secs(self.config.capture.interval_minutes * 60);

        // Spawn screenshot capture loop
        let db_capture = db.clone();
        let capture_config = self.config.clone();
        let capture_handle = tokio::spawn(async move {
            let mut timer = interval_at(Instant::now() + capture_interval, capture_interval);
            loop {
                timer.tick().await;

                if Config::is_paused() {
                    tracing::debug!("Captures paused, skipping");
                    continue;
                }

                if !capture_config.is_within_active_hours() {
                    tracing::debug!(
                        "Outside active hours ({}-{}), skipping",
                        capture_config.capture.active_hours_start,
                        capture_config.capture.active_hours_end
                    );
                    continue;
                }

                match capturer.take_screenshots(&capture_config.capture) {
                    Ok(captures) => {
                        // Process each display's screenshot sequentially (GPU can only do one at a time)
                        for capture in captures {
                            let file_path_str = capture.file_path.to_string_lossy().to_string();
                            let id = {
                                let db = db_capture.lock().await;
                                match db.insert_screenshot(
                                    &capture.captured_at,
                                    &file_path_str,
                                    &capture.display_label,
                                ) {
                                    Ok(id) => id,
                                    Err(e) => {
                                        tracing::error!("DB insert failed: {}", e);
                                        continue;
                                    }
                                }
                            };
                            // Lock is dropped here before the await point

                            // Process with Ollama (no mutex held across this await)
                            let result = processor
                                .process_screenshot(&capture.file_path, &capture.display_label)
                                .await;
                            match result {
                                Ok((summary, model, method)) => {
                                    let db = db_capture.lock().await;
                                    if let Err(e) =
                                        db.update_screenshot_summary(id, &summary, &model, &method)
                                    {
                                        tracing::error!("DB update failed: {}", e);
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Processing failed for {}: {}", capture.display_label, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Screenshot capture failed: {}", e);
                    }
                }
            }
        });

        // Spawn daily summary loop — checks every 30s instead of sleeping for hours
        let db_summary = db.clone();
        let daily_time = self.config.summary.daily_time.clone();
        let summary_handle = tokio::spawn(async move {
            let (target_h, target_m) = Self::parse_daily_time(&daily_time);
            let mut last_summary_date: Option<String> = None;
            let mut retry_count: u32 = 0;
            const MAX_RETRIES: u32 = 5;
            let mut check_interval = tokio::time::interval(Duration::from_secs(30));

            tracing::info!("Summary loop started, target time {}:{:02}", target_h, target_m);

            loop {
                check_interval.tick().await;

                let now = Local::now();
                let today = now.format("%Y-%m-%d").to_string();
                let current_h = now.hour();
                let current_m = now.minute();

                // Skip if we already generated a summary for today
                if last_summary_date.as_deref() == Some(today.as_str()) {
                    continue;
                }

                // Only trigger when current time is at or past the target
                if current_h < target_h || (current_h == target_h && current_m < target_m) {
                    continue;
                }

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
                    last_summary_date = Some(today.clone());
                    continue;
                }

                let screenshot_count = entries.len() as i64;

                match summarizer.generate_daily_summary(&today, &entries).await {
                    Ok((content, file_path)) => {
                        let file_path_str = file_path.to_string_lossy().to_string();
                        {
                            let db = db_summary.lock().await;
                            if let Err(e) = db.insert_daily_summary(
                                &today,
                                &content,
                                &file_path_str,
                                screenshot_count,
                            ) {
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

                        last_summary_date = Some(today.clone());
                        retry_count = 0;
                        tracing::info!("Daily summary generated for {}", today);
                    }
                    Err(e) => {
                        retry_count += 1;
                        tracing::error!(
                            "Failed to generate daily summary (attempt {}/{}): {}",
                            retry_count, MAX_RETRIES, e
                        );
                        if retry_count >= MAX_RETRIES {
                            tracing::error!(
                                "Giving up on daily summary for {} after {} attempts",
                                today, MAX_RETRIES
                            );
                            last_summary_date = Some(today.clone());
                            retry_count = 0;
                        }
                    }
                }
            }
        });

        // Set up signal handlers for graceful shutdown
        let mut sigterm = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        )?;

        // Wait for tasks or shutdown signal
        tokio::select! {
            _ = capture_handle => tracing::error!("Capture loop ended unexpectedly"),
            _ = summary_handle => tracing::error!("Summary loop ended unexpectedly"),
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received SIGINT, shutting down gracefully");
            }
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM, shutting down gracefully");
            }
        }

        // Clean up PID file on exit
        Self::remove_pid();
        tracing::info!("Sauron stopped");

        Ok(())
    }

    fn check_screen_permission(&self) -> anyhow::Result<()> {
        let granted = unsafe { CGPreflightScreenCaptureAccess() };
        if granted {
            tracing::info!("Screen recording permission granted");
            return Ok(());
        }

        tracing::warn!("Screen recording permission not granted, requesting access...");
        let requested = unsafe { CGRequestScreenCaptureAccess() };

        if !requested {
            anyhow::bail!(
                "Screen recording permission denied. \
                 Please grant access in System Settings → Privacy & Security → Screen Recording, \
                 then restart sauron."
            );
        }

        Ok(())
    }

    fn parse_daily_time(time_str: &str) -> (u32, u32) {
        let parts: Vec<u32> = time_str.split(':').filter_map(|s| s.parse().ok()).collect();
        (
            parts.first().copied().unwrap_or(23),
            parts.get(1).copied().unwrap_or(59),
        )
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
