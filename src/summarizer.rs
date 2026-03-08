use reqwest::Client;
use serde_json::json;
use std::path::PathBuf;

pub struct Summarizer {
    client: Client,
    base_url: String,
    text_model: String,
    output_dir: PathBuf,
}

impl Summarizer {
    pub fn new(base_url: String, text_model: String, output_dir: PathBuf) -> Self {
        Self {
            client: Client::new(),
            base_url,
            text_model,
            output_dir,
        }
    }

    /// Generate a daily summary from individual screenshot summaries
    /// Returns (markdown_content, file_path)
    pub async fn generate_daily_summary(
        &self,
        date: &str,
        entries: &[(String, String)], // (captured_at, summary)
    ) -> anyhow::Result<(String, PathBuf)> {
        if entries.is_empty() {
            anyhow::bail!("No entries to summarize for {}", date);
        }

        let timeline = entries
            .iter()
            .map(|(time, summary)| {
                let time_short = time
                    .split('T')
                    .nth(1)
                    .and_then(|t| t.get(..5))
                    .unwrap_or(time);
                format!("[{}] {}", time_short, summary)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Based on these chronological screen activity summaries from {}, \
            create a concise daily activity log in markdown format. \
            Group related activities, note time ranges, highlight key accomplishments. \
            Use headers and bullet points.\n\n{}",
            date, timeline
        );

        let url = format!("{}/api/generate", self.base_url);
        let body = json!({
            "model": self.text_model,
            "prompt": prompt,
            "stream": false,
        });

        let resp = self.client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(180))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama returned status: {}", resp.status());
        }

        let json: serde_json::Value = resp.json().await?;
        let content = json["response"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No response field"))?
            .trim()
            .to_string();

        // Add header
        let markdown = format!("# Daily Activity Log — {}\n\n{}\n", date, content);

        // Save to filesystem
        std::fs::create_dir_all(&self.output_dir)?;
        let file_path = self.output_dir.join(format!("{}.md", date));
        std::fs::write(&file_path, &markdown)?;
        tracing::info!("Daily summary saved: {}", file_path.display());

        Ok((markdown, file_path))
    }
}
