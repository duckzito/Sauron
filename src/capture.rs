use std::path::PathBuf;
use std::process::Command;
use chrono::Local;

use crate::config::CaptureConfig;

extern "C" {
    fn CGGetActiveDisplayList(
        max_displays: u32,
        active_displays: *mut u32,
        display_count: *mut u32,
    ) -> i32;
}

/// Returns the number of active displays via CoreGraphics.
fn get_active_display_count() -> u32 {
    let mut count: u32 = 0;
    let status = unsafe { CGGetActiveDisplayList(0, std::ptr::null_mut(), &mut count) };
    if status != 0 || count == 0 {
        tracing::warn!(
            "CGGetActiveDisplayList failed (status={}), defaulting to 1 display",
            status
        );
        return 1;
    }
    count
}

#[allow(dead_code)]
pub struct CaptureResult {
    pub captured_at: String,
    pub file_path: PathBuf,
    pub display_index: usize,
    pub display_label: String,
}

pub struct Capturer {
    screenshot_dir: PathBuf,
}

impl Capturer {
    pub fn new(screenshot_dir: PathBuf) -> Self {
        Self { screenshot_dir }
    }

    /// Take a screenshot of each connected display.
    /// Returns a vec of capture results (one per display that succeeded).
    pub fn take_screenshots(
        &self,
        capture_config: &CaptureConfig,
    ) -> anyhow::Result<Vec<CaptureResult>> {
        let total_displays = get_active_display_count() as usize;

        // Determine which displays to capture
        let display_indices: Vec<usize> = match &capture_config.monitors {
            Some(monitors) => monitors
                .iter()
                .copied()
                .filter(|&i| i < total_displays)
                .collect(),
            None => (0..total_displays).collect(),
        };

        if display_indices.is_empty() {
            anyhow::bail!(
                "No valid displays to capture (total={}, filter={:?})",
                total_displays,
                capture_config.monitors
            );
        }

        // Compute timestamp once so all captures in a cycle share the same time
        let now = Local::now();
        let date_dir = now.format("%Y-%m-%d").to_string();
        let time_stem = now.format("%H-%M-%S").to_string();
        let captured_at = now.to_rfc3339();

        let dir = self.screenshot_dir.join(&date_dir);
        std::fs::create_dir_all(&dir)?;

        let mut results = Vec::new();

        for &idx in &display_indices {
            let display_label = format!("display-{}", idx);
            let filename = format!("{}_{}.png", time_stem, display_label);
            let file_path = dir.join(&filename);

            // screencapture -D is 1-based
            let display_num = (idx + 1).to_string();

            let file_path_str = file_path.to_string_lossy();
            let status = Command::new("screencapture")
                .args(["-x", "-D", &display_num, file_path_str.as_ref()])
                .status();

            match status {
                Ok(s) if s.success() => {
                    tracing::info!(
                        "Screenshot saved: {} ({})",
                        file_path.display(),
                        display_label
                    );
                    results.push(CaptureResult {
                        captured_at: captured_at.clone(),
                        file_path,
                        display_index: idx,
                        display_label,
                    });
                }
                Ok(s) => {
                    tracing::error!(
                        "screencapture for display {} exited with status: {}",
                        idx,
                        s
                    );
                }
                Err(e) => {
                    tracing::error!("screencapture for display {} failed: {}", idx, e);
                }
            }
        }

        if results.is_empty() {
            anyhow::bail!("All display captures failed");
        }

        Ok(results)
    }
}
