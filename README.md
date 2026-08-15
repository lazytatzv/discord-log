# discord-log

A high-performance, developer-friendly Rust CLI tool to stream/upload terminal execution logs directly to a Discord Webhook.

## Features

- **Transparent Command Execution**: Run `log <command>` directly. Streams output in real-time to terminal while capturing stdout & stderr.
- **Pipe & File Upload Support**: Compatible with traditional pipe streams (`colcon build 2>&1 | log`) or log file uploads (`log -f build.log`).
- **Ctrl+C Safe**: Captures and uploads logs up to the exact moment of interruption with `[INTERRUPTED]` status.
- **Clean Output**: No emojis, minimal text format (`[SUCCESS]`, `[FAILED]`, `[INTERRUPTED]`).

## Installation

```bash
cargo install --git https://github.com/lazytatzv/discord-log.git
```

Or clone and build locally:

```bash
git clone git@github.com:lazytatzv/discord-log.git
cd discord-log
cargo install --path .
```

To use `log` command name directly:
```bash
ln -sf ~/.cargo/bin/discord-log ~/.cargo/bin/log
```

## Configuration

Set your target Discord channel Webhook URL in your shell config (`~/.bashrc` or `~/.zshrc`):

```bash
export DISCORD_WEBHOOK_URL="https://discord.com/api/webhooks/your/webhook/url"
```

## Usage

### 1. Wrap Command Execution (Recommended)

```bash
log colcon build
```

### 2. Pipe Input Stream

```bash
colcon build 2>&1 | log
```

### 3. Send Existing Log File

```bash
log -f /path/to/build.log
```

### 4. Custom Title / Username

```bash
log -t "Build Task" -u "CI-Bot" colcon build
```

## Options

```text
Usage: log [OPTIONS] [COMMAND]...

Arguments:
  [COMMAND]...  Command to execute and capture

Options:
  -w, --webhook <WEBHOOK>    Target Discord Webhook URL (env: DISCORD_WEBHOOK_URL)
  -f, --file <FILE>          Send an existing log file
  -t, --title <TITLE>        Title / Summary tag for the log
  -u, --username <USERNAME>  Custom Bot name [default: "Log Bot"]
      --err-only             Capture ONLY stderr for Discord
      --out-only             Capture ONLY stdout for Discord
  -h, --help                 Print help
  -V, --version              Print version
```
