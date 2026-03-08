# Sauron

A macOS screen activity tracker that periodically captures screenshots, analyzes them with local AI via [Ollama](https://ollama.com), and generates daily productivity summaries. All processing happens locally on your machine.

## How It Works

1. **Capture** — Takes a screenshot every 3 minutes (configurable) during active hours
2. **Analyze** — Sends each screenshot to an Ollama vision model (llava) to describe what you're working on; falls back to OCR if the vision model is unavailable
3. **Summarize** — At end of day, aggregates all descriptions and generates a markdown summary via an Ollama text model (llama3.2)
4. **Report** — Saves the summary to disk and optionally emails it via [Resend](https://resend.com)

## Prerequisites

- **macOS 13+** (Ventura or later)
- **Rust** toolchain (`rustup`)
- **Xcode Command Line Tools** (for Swift menu bar app)
- **Ollama** running locally with models pulled:
  ```bash
  ollama pull llava
  ollama pull llama3.2
  ```
- **Screen Recording permission** — Sauron will prompt on first run

## Installation

```bash
# Build daemon + menu bar app
make build

# Install to system (requires sudo)
make install
```

This installs the `sauron` binary to `/usr/local/bin`, the menu bar app to `/Applications`, and sets up launchd services for auto-start.

To remove everything:

```bash
make uninstall
```

## Usage

### CLI

```bash
sauron start          # Start daemon in foreground
sauron stop           # Stop running daemon
sauron status         # Show running state and statistics
sauron install        # Install launchd auto-start service
sauron uninstall      # Remove launchd service
sauron summary        # Generate today's summary now
sauron summary --date 2026-03-08   # Generate summary for a specific date
sauron config         # Print current configuration
```

### Menu Bar App

After installation, **SauronMenu** appears as an eye icon in the macOS menu bar. It shows:

- Daemon status (running / paused / stopped)
- Last capture time and today's screenshot count
- Controls to start/stop, pause/resume, trigger a summary, and open the config file

## Configuration

Create `~/.config/sauron/config.toml` to override defaults:

```toml
[capture]
interval_minutes = 3               # Screenshot interval
screenshot_dir = "~/sauron/screenshots"
active_hours_start = "09:00"       # Only capture during these hours
active_hours_end = "18:00"

[ollama]
base_url = "http://localhost:11434"
vision_model = "llava"             # Model for screenshot analysis
text_model = "llama3.2"            # Model for daily summaries

[summary]
daily_time = "23:59"               # When to auto-generate daily summary
output_dir = "~/sauron/daily-logs"

[email]                            # Optional — omit to disable email
resend_api_key = "re_xxxxxxxxxxxx"
from = "sauron@yourdomain.com"
to = "you@yourdomain.com"

[database]
path = "~/sauron/sauron.db"
```

All settings have sensible defaults. Sauron works with zero configuration as long as Ollama is running.

## Data Storage

- **Screenshots** — PNG files in `~/sauron/screenshots/` (organized by date)
- **Daily summaries** — Markdown files in `~/sauron/daily-logs/`
- **Database** — SQLite at `~/sauron/sauron.db` (metadata index)
- **Config & state** — `~/.config/sauron/` (config, PID file, pause file)

## License

MIT
