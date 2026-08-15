# discord-log (`log`)

Send command execution outputs, build logs, and files directly to a Discord channel cleanly and reliably.

## Features

- **Command Wrapper**: Attach `log` before any command (`log make`, `log cargo build`).
- **Comment Support**: Attach custom messages/comments to your logs (`log -c "Raspberry Pi Build" make`).
- **Log Download**: Download recent logs back to your terminal (`log dl`).
- **File Attachment**: Upload files directly (`log -f screenshot.png`).
- **Grep Filtering**: Filter output lines by keyword before sending (`log -g error colcon build`).
- **Zero Setup Local History**: Automatically saves execution history locally as fallback.

## Installation

```bash
cargo install discord-log
```

## Quick Setup

```bash
# 1. Save your Discord Webhook URL (Required)
log --init "https://discord.com/api/webhooks/YOUR_WEBHOOK_ID/YOUR_WEBHOOK_TOKEN"

# 2. Save your Discord Bot Token (Optional: Required for fetching history via Discord API)
log --init-token "YOUR_DISCORD_BOT_TOKEN"
```

## Usage & Examples

### Basic Command Execution
```bash
log colcon build
log make -j4
```

### Adding Comments / Messages
```bash
log -c "Raspberry Pi Kernel Build" make -j4
log -m "Running unit tests" pytest
```

### Downloading Recent Logs
```bash
log dl       # Download the latest log
log dl 1     # Download 2nd latest log (1 step back)
log dl 2     # Download 3rd latest log (2 steps back)
```

### Uploading Files
```bash
log -f screenshot.png
log -f build.log
```

### Filtering Output Lines
```bash
log -g error colcon build
```

### Output Redirection Options
```bash
log -e make    # Send stderr output ONLY
log -o pytest  # Send stdout output ONLY
```

## Options

| Option | Description |
| :--- | :--- |
| `-c, --comment <msg>` | Add a custom comment/message to the log output |
| `-g, --grep <word>` | Filter output lines matching keyword (case-insensitive) |
| `-f, --file <path>` | Upload file directly as attachment |
| `-e, --stderr` | Send ONLY stderr output |
| `-o, --stdout` | Send ONLY stdout output |
| `-w, --webhook <URL>`| Override configured Discord Webhook URL |
| `--init <URL>` | Save Discord Webhook URL (one-time setup) |
| `--init-token <token>`| Save Discord Bot Token |
| `-h, --help` | Show help message |
| `-V, --version` | Show version |
