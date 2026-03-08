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
