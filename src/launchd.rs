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
