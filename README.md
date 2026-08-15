# discord-log (log)

Send command outputs or files directly to Discord.

## Installation

```bash
cargo install discord-log
```

## Quick Setup (Run once)

```bash
log --init "https://discord.com/api/webhooks/your/webhook/url"
```

*(Optional: Set Bot Token to fetch past logs directly from Discord)*
```bash
log --init-token "YOUR_DISCORD_BOT_TOKEN"
```

## Usage

```bash
# Run any command & send output
log colcon build
log make -j4

# Download latest log (or N steps back)
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
