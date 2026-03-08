# Sauron — Screen Activity Logger

## Overview

A Rust CLI tool for macOS that captures screenshots every 3 minutes, generates LLM-powered summaries via Ollama, and produces daily activity logs sent by email.

## Architecture

Single async binary (tokio) with internal modules:

- **capture** — takes screenshots via macOS `screencapture -x`
- **processor** — sends screenshots to Ollama (vision model primary, OCR + text model fallback)
- **store** — SQLite (index/metadata) + filesystem (files)
- **summarizer** — generates daily summary, saves markdown, sends email via Resend
- **scheduler** — tokio timers (3-min capture, 23:59 daily summary)
- **config** — TOML config parsing
- **cli** — subcommands via clap

## Data Flow

```
every 3 min:  screencapture → save PNG to disk → Ollama vision/OCR → store summary + path in SQLite
at 23:59:     query day's summaries from SQLite → Ollama generates day log → save .md → store in SQLite → send via Resend
```

## SQLite Schema

```sql
CREATE TABLE screenshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    captured_at TEXT NOT NULL,
    file_path TEXT NOT NULL,
    summary TEXT,
    model_used TEXT,
    processing_method TEXT,
    processed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE daily_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    summary_date TEXT NOT NULL UNIQUE,
    content TEXT NOT NULL,
    file_path TEXT NOT NULL,
    screenshot_count INTEGER,
    email_sent_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

## Config File

Location: `~/.config/sauron/config.toml`

```toml
[capture]
interval_minutes = 3
screenshot_dir = "~/sauron/screenshots"

[ollama]
base_url = "http://localhost:11434"
vision_model = "llava"
text_model = "llama3.2"

[summary]
daily_time = "23:59"
output_dir = "~/sauron/daily-logs"

[email]
resend_api_key = "re_xxxxxxxxxxxx"
from = "sauron@yourdomain.com"
to = "you@yourdomain.com"

[database]
path = "~/sauron/sauron.db"
```

All fields have defaults except `email.resend_api_key`, `email.from`, and `email.to`.

## CLI Interface

```
sauron start          # Start daemon in foreground
sauron stop           # Stop running daemon via PID file
sauron status         # Show running state, last screenshot, today's count
sauron install        # Install launchd plist for auto-start
sauron uninstall      # Remove launchd plist
sauron summary        # Manually trigger daily summary (--date YYYY-MM-DD)
sauron config         # Print current config
```

Daemon writes PID to `~/.config/sauron/sauron.pid`. `install` generates `com.sauron.agent.plist` in `~/Library/LaunchAgents/`.

## Screenshot Capture

- `screencapture -x` (silent) via `std::process::Command`
- Saved to `screenshot_dir/YYYY-MM-DD/HH-MM-SS.png`
- Row inserted into `screenshots` table immediately (summary = null)

## Processing Pipeline

1. Try vision model: POST screenshot as base64 to Ollama `/api/generate`
2. If vision fails: OCR via macOS Vision.framework (Swift helper) or tesseract CLI, then send extracted text to text model
3. If both fail: log error, leave summary null, continue

Prompt: "Describe what the user is doing on their screen. Be concise (2-3 sentences). Focus on: application in use, task being performed, key content visible."

## Daily Summary

1. Query all screenshots for the day with non-null summaries, ordered by time
2. Build chronological timeline, send to Ollama text model with prompt to create a daily activity log
3. Save as `output_dir/YYYY-MM-DD.md`
4. Insert into `daily_summaries` table
5. Send via Resend API (`POST https://api.resend.com/emails`)
6. Update `email_sent_at`

Failures are logged but don't block: summary saved even if email fails; pipeline continues even if Ollama is temporarily down.

## Key Crates

- `tokio` — async runtime & scheduling
- `rusqlite` — SQLite
- `reqwest` — HTTP (Ollama + Resend)
- `serde` / `toml` — config
- `clap` — CLI
- `chrono` — datetime

## Platform

macOS only. Uses `screencapture` CLI and optionally macOS Vision.framework for OCR fallback.

## Storage

Dual storage strategy:
- **Filesystem:** screenshots as PNG files, daily summaries as markdown files
- **SQLite:** structured index with metadata, summaries, and file paths

## Decisions

- Monolithic single binary over multi-process (simpler for single-user local tool)
- User-configurable Ollama models (no hardcoded model names)
- Vision model primary with OCR fallback for resilience
- Resend for email delivery
- launchd integration for persistence with CLI for manual control
