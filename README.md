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

## Usage

```bash
# Run any command & send output
log colcon build
log make -j4

# Upload any file (.txt, .png, .pdf, etc.)
log -f screenshot.png
log -f build.log

# Filter lines matching keyword
log -g error colcon build

# Send stderr / stdout only
log -e make
log -o pytest
```
