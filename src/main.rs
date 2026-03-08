mod capture;
mod cli;
mod config;
mod daemon;
mod db;
mod email;
mod error;
mod processor;
mod summarizer;

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
