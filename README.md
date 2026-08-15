# discord-log (log)

Send command execution logs directly to Discord.

## Usage

```bash
# 1. Default (stdout + stderr)
log colcon build

# 2. stderr only
log -e colcon build

# 3. stdout only
log -o colcon build
```

## Setup

```bash
export DISCORD_WEBHOOK_URL="https://discord.com/api/webhooks/your/webhook/url"
```

Install via Cargo:
```bash
cargo install --git https://github.com/lazytatzv/discord-log.git
ln -sf ~/.cargo/bin/discord-log ~/.cargo/bin/log
```
