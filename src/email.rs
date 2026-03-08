use reqwest::Client;
use serde_json::json;

pub struct Mailer {
    client: Client,
    api_key: String,
    from: String,
    to: String,
}

impl Mailer {
    pub fn new(api_key: String, from: String, to: String) -> Option<Self> {
        if api_key.is_empty() || from.is_empty() || to.is_empty() {
            tracing::warn!("Email not configured — daily summaries will not be emailed");
            return None;
        }
        Some(Self {
            client: Client::new(),
            api_key,
            from,
            to,
        })
    }

    pub async fn send_daily_summary(&self, date: &str, markdown_content: &str) -> anyhow::Result<()> {
        let subject = format!("Sauron Daily Log — {}", date);

        // Convert markdown to simple HTML (basic conversion)
        let html_content = format!(
            "<html><body><pre style=\"font-family: monospace; white-space: pre-wrap;\">{}</pre></body></html>",
            html_escape(markdown_content)
        );

        let body = json!({
            "from": self.from,
            "to": [self.to],
            "subject": subject,
            "html": html_content,
        });

        let resp = self.client
            .post("https://api.resend.com/emails")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Resend API error ({}): {}", status, text);
        }

        tracing::info!("Daily summary email sent for {}", date);
        Ok(())
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
