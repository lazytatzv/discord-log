# discord-log (log)

A CLI tool that wraps **any command** (`make`, `colcon build`, `pytest`, `cargo build`, etc.) and automatically streams the execution output to a Discord Webhook.

## Installation

```bash
cargo install discord-log
```

## Quick Setup (Run once)

```bash
log --init "https://discord.com/api/webhooks/your/webhook/url"
```

## Usage

Simply prefix `log` to **any command**:

```bash
# Works with ANY command:
log colcon build
log make -j4
log cargo build --release
log pytest

# Filter output lines by keyword (case-insensitive):
log -g error colcon build

# Send stderr only:
log -e make

# Send stdout only:
log -o pytest

# Combine flags (e.g. stderr only + grep error):
log -e -g error colcon build
```
