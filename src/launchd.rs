use crate::config::Config;
use std::path::PathBuf;

const PLIST_LABEL: &str = "com.sauron.agent";
const MENUBAR_PLIST_LABEL: &str = "com.sauron.menubar";

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

    // Install menu bar app launchd agent
    install_menubar()?;

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

    // Uninstall menu bar app
    uninstall_menubar()?;

    Ok(())
}

fn install_menubar() -> anyhow::Result<()> {
    let menubar_plist = menubar_plist_path();

    let app_bundle = find_menubar_app();
    let Some(app_bundle) = app_bundle else {
        tracing::warn!(
            "SauronMenu.app not found, skipping menu bar app installation. \
             Build it with: make build-menubar"
        );
        return Ok(());
    };

    let binary = app_bundle.join("Contents/MacOS/SauronMenu");

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
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>"#,
        MENUBAR_PLIST_LABEL,
        binary.display(),
    );

    std::fs::write(&menubar_plist, plist_content)?;

    let status = std::process::Command::new("launchctl")
        .args(["load", menubar_plist.to_str().unwrap()])
        .status()?;

    if status.success() {
        tracing::info!("Menu bar app installed and loaded");
    } else {
        tracing::warn!("Failed to load menu bar app launchd service");
    }

    Ok(())
}

fn uninstall_menubar() -> anyhow::Result<()> {
    let menubar_plist = menubar_plist_path();

    if menubar_plist.exists() {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", menubar_plist.to_str().unwrap()])
            .status();

        // Kill any running SauronMenu process
        let _ = std::process::Command::new("pkill")
            .args(["-f", "SauronMenu"])
            .status();

        std::fs::remove_file(&menubar_plist)?;
        tracing::info!("Menu bar app uninstalled");
    }

    Ok(())
}

fn find_menubar_app() -> Option<PathBuf> {
    // Check /Applications
    let global = PathBuf::from("/Applications/SauronMenu.app");
    if global.exists() {
        return Some(global);
    }

    // Check ~/Applications
    if let Some(home) = dirs::home_dir() {
        let user_app = home.join("Applications/SauronMenu.app");
        if user_app.exists() {
            return Some(user_app);
        }
    }

    None
}

fn plist_path() -> PathBuf {
    launch_agents_dir().join(format!("{}.plist", PLIST_LABEL))
}

fn menubar_plist_path() -> PathBuf {
    launch_agents_dir().join(format!("{}.plist", MENUBAR_PLIST_LABEL))
}

fn launch_agents_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join("Library/LaunchAgents")
}
