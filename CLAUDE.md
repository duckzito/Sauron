# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Sauron

A macOS productivity tracker that periodically captures screenshots, analyzes them with local AI (Ollama), generates daily summaries, and optionally emails reports. Consists of a Rust async daemon and a Swift menu bar app.

## Build & Run Commands

```bash
# Build everything (daemon + menu bar app)
make build

# Build only the Rust daemon
cargo build --release

# Build only the Swift menu bar app
cd menubar && swift build -c release

# Install to system (daemon → /usr/local/bin, menu bar → /Applications, sets up launchd)
make install

# Uninstall
make uninstall

# Clean build artifacts
make clean

# Run tests
cargo test

# Run clippy
cargo clippy

# Run daemon in foreground
cargo run -- start

# Trigger a manual daily summary
cargo run -- summary --date 2026-03-08
```

## Architecture

**Rust daemon** (`src/`) — Tokio-based async daemon with two concurrent loops:
- **Capture loop** (`daemon.rs` → `capture.rs` → `processor.rs`): Takes screenshots at configurable intervals, processes them through Ollama vision model (with OCR fallback via macOS Vision.framework), stores results in SQLite.
- **Summary loop** (`daemon.rs` → `summarizer.rs` → `email.rs`): At configured daily time, aggregates the day's screenshot summaries into a markdown report via Ollama text model, saves to disk, and optionally emails via Resend API.

**Swift menu bar app** (`menubar/`) — SwiftUI app that reads daemon state (PID file, SQLite) and controls it via CLI subcommands. Refreshes every 10 seconds.

### Key Data Flow

```
screencapture → PNG file → Ollama vision (or OCR fallback) → per-screenshot summary → SQLite
                                                                                        ↓
                                          Email ← Markdown file ← Ollama text model ← day's summaries
```

### Database (SQLite)

Two tables in `db.rs`:
- `screenshots`: file_path, summary, model_used, processing_method, timestamps
- `daily_summaries`: summary_date (unique), content, file_path, screenshot_count, email_sent_at

### Configuration

TOML config at `~/.config/sauron/config.toml` with sections: `[capture]`, `[ollama]`, `[summary]`, `[email]`, `[database]`. All paths support `~` expansion (handled in `config.rs`).

### macOS Integration

- Screen recording permission via CoreGraphics C bindings (`build.rs` links CoreGraphics)
- PID file at `~/.config/sauron/sauron.pid`; pause file `sauron.paused` in same dir
- launchd plists: `com.sauron.agent` (daemon), `com.sauron.menubar` (menu bar)
- CLI parsing via clap (`cli.rs`): start, stop, status, install, uninstall, summary, config

## Key Dependencies

- **tokio** (async runtime), **clap** (CLI), **rusqlite** (SQLite), **reqwest** (HTTP/Ollama/Resend), **tracing** (logging), **chrono** (time), **thiserror** (error types)
