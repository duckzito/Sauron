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
