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
use daemon::{is_sauron_process, Daemon};
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
                if is_sauron_process(pid) {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                    Daemon::remove_pid();
                    println!("Sauron stopped (PID {})", pid);
                } else {
                    Daemon::remove_pid();
                    println!("Sauron is not running (stale PID file cleaned, PID {} belongs to another process)", pid);
                }
            } else {
                println!("Sauron is not running");
            }
        }

        Commands::Status => {
            if let Some(pid) = Daemon::read_pid() {
                // Check if process is actually running
                let running = unsafe { libc::kill(pid as i32, 0) == 0 };
                if running && is_sauron_process(pid) {
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
