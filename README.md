# discord-log (log)

A CLI tool that wraps **any command** (`make`, `colcon build`, `pytest`, `cargo build`, etc.) or uploads log files directly to a Discord Webhook.

## Installation

```bash
cargo install discord-log
```

## Quick Setup (Run once)

```bash
log --init "https://discord.com/api/webhooks/your/webhook/url"
```

## Usage

### 1. Run ANY command & stream logs to Discord

```bash
log colcon build
log make -j4
log cargo build --release
log pytest
```

### 2. Upload an existing log file directly

```bash
log -f path/to/build.log
```

### 3. Filters & Options

```bash
# Filter lines matching keyword (case-insensitive):
log -g error colcon build
log -f build.log -g error

# Send stderr only:
log -e make

# Send stdout only:
log -o pytest
```
