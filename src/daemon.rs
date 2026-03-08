use crate::capture::Capturer;
use crate::config::Config;
use crate::db::Database;
use crate::email::Mailer;
use crate::processor::Processor;
use crate::summarizer::Summarizer;

use chrono::Local;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval_at, Duration, Instant};

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
            let mut timer = interval_at(Instant::now() + capture_interval, capture_interval);
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
                        // Lock is dropped here before the await point

                        // Process with Ollama (no mutex held across this await)
                        let result = processor.process_screenshot(&file_path).await;
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
                    }
                    Err(e) => {
                        tracing::error!("Failed to generate daily summary: {}", e);
                    }
                }

                // Sleep briefly before recalculating duration_until to avoid
                // edge cases where we're still at the target minute
                tokio::time::sleep(Duration::from_secs(60)).await;
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

    fn duration_until(time_str: &str) -> Duration {
        let parts: Vec<u32> = time_str.split(':').filter_map(|s| s.parse().ok()).collect();
        let (target_h, target_m) = (
            parts.first().copied().unwrap_or(23),
            parts.get(1).copied().unwrap_or(59),
        );

        let now = Local::now();
        let today_target = now
            .date_naive()
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
