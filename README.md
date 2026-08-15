# discord-log (log)

Send command execution logs directly to Discord.

## Quick Setup (Run once)

```bash
log --init "https://discord.com/api/webhooks/your/webhook/url"
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

## Installation

```bash
cargo install --git https://github.com/lazytatzv/discord-log.git
```
