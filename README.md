# discord-log (log)

Send command outputs or files directly to Discord.

## Installation

```bash
cargo install discord-log
```

## Quick Setup (Run once)

```bash
# Set Discord Webhook URL (Required for sending logs)
log --init "https://discord.com/api/webhooks/your/webhook/url"

# Set Discord Bot Token (Optional: Required for 'log dl N' history traversal)
log --init-token "YOUR_DISCORD_BOT_TOKEN"
```

## Usage

```bash
# Run any command & send output
log colcon build
log make -j4

# Add a comment / message to the output
log -c "Raspberry Pi Kernel Build" make -j4
log -m "Unit Test Results" pytest

# Download logs from Discord
log dl       # Download latest log
log dl 1     # Download 2nd latest log (1 step back)
log dl 2     # Download 3rd latest log (2 steps back)

# Upload any file (.txt, .png, .pdf, etc.)
log -f screenshot.png
log -f build.log

# Filter lines matching keyword
log -g error colcon build

# Send stderr / stdout only
log -e make
log -o pytest
```
