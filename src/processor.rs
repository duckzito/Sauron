use base64::Engine;
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
    pub async fn process_screenshot(
        &self,
        image_path: &Path,
        display_label: &str,
    ) -> anyhow::Result<(String, String, String)> {
        // Try vision model first
        match self.try_vision(image_path, display_label).await {
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

    async fn try_vision(&self, image_path: &Path, display_label: &str) -> anyhow::Result<String> {
        let image_bytes = tokio::fs::read(image_path).await?;
        let image_b64 = base64::engine::general_purpose::STANDARD.encode(&image_bytes);

        let prompt = format!(
            "{} This screenshot is from monitor '{}'.",
            SCREENSHOT_PROMPT, display_label
        );

        let url = format!("{}/api/generate", self.base_url);
        let body = json!({
            "model": self.vision_model,
            "prompt": prompt,
            "images": [image_b64],
            "stream": false,
        });

        let resp = self
            .client
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
        let swift_script = r#"
import Vision
import AppKit

let imagePath = CommandLine.arguments[1]
let url = URL(fileURLWithPath: imagePath)
guard let image = NSImage(contentsOf: url),
      let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
    print("ERROR: Could not load image")
    exit(1)
}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
try handler.perform([request])

let text = (request.results ?? [])
    .compactMap { $0.topCandidates(1).first?.string }
    .joined(separator: "\n")
print(text)
"#;

        let path_str = image_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Image path is not valid UTF-8"))?;

        let output = tokio::process::Command::new("swift")
            .args(["-e", swift_script, path_str])
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

        let resp = self
            .client
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
