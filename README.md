# discord-log (log)

Send command execution outputs to a Discord Webhook.

## Installation

```bash
cargo install --git https://github.com/lazytatzv/discord-log.git
```

## Quick Setup (Run once)

```bash
log --init "https://discord.com/api/webhooks/your/webhook/url"
```

## Usage

```bash
# Default (stdout + stderr)
log colcon build

# Send stderr only
log -e colcon build

# Send stdout only
log -o colcon build
```
