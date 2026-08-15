# discord-log (log)

Send command execution logs directly to Discord.

## Installation

```bash
cargo install --git https://github.com/lazytatzv/discord-log.git
```
*(Installs both `log` and `discord-log` commands automatically)*

## Setup

Set your target Discord channel Webhook URL in your shell config (`~/.bashrc` or `~/.zshrc`):

```bash
export DISCORD_WEBHOOK_URL="https://discord.com/api/webhooks/your/webhook/url"
```

## Usage

```bash
# 1. Default (stdout + stderr)
log colcon build

# 2. stderr only
log -e colcon build

# 3. stdout only
log -o colcon build
```
