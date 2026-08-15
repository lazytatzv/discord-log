use anyhow::{bail, Context, Result};
use reqwest::multipart;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const HELP_TEXT: &str = r#"discord-log (log) - Send command execution outputs or download logs to/from Discord.

Usage:
  log <command> [args...]
  log dl [URL]       Download specified URL or automatically fetch from clipboard

Options:
  --init <URL>       Save Discord Webhook URL (one-time setup)
  -e, --stderr       Send ONLY stderr output
  -o, --stdout       Send ONLY stdout output
  -g, --grep <word>  Filter output lines matching keyword (case-insensitive)
  -f, --file <path>  Upload file directly as attachment
  -w, --webhook <URL> Override Discord Webhook URL
  -h, --help         Show this help message
  -V, --version      Show version

Examples:
  log colcon build
  log dl             # Auto-detects URL from clipboard & downloads instantly!
  log dl "https://cdn.discordapp.com/attachments/..."
"#;

struct Config {
    webhook: Option<String>,
    file: Option<PathBuf>,
    stderr: bool,
    stdout: bool,
    grep: Option<String>,
    command: Vec<String>,
    show_help: bool,
    show_version: bool,
    is_dl: bool,
    dl_url: Option<String>,
}

fn parse_custom_args() -> Config {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut webhook = None;
    let mut file = None;
    let mut stderr = false;
    let mut stdout = false;
    let mut grep = None;
    let mut command = Vec::new();
    let mut show_help = false;
    let mut show_version = false;
    let mut is_dl = false;
    let mut dl_url = None;

    if !raw.is_empty() && raw[0] == "dl" {
        is_dl = true;
        if raw.len() > 1 && !raw[1].starts_with('-') {
            dl_url = Some(raw[1].clone());
        }
        return Config {
            webhook,
            file,
            stderr,
            stdout,
            grep,
            command,
            show_help,
            show_version,
            is_dl,
            dl_url,
        };
    }

    let mut idx = 0;
    while idx < raw.len() {
        let arg = &raw[idx];
        if arg == "-h" || arg == "--help" {
            show_help = true;
            idx += 1;
        } else if arg == "-V" || arg == "--version" {
            show_version = true;
            idx += 1;
        } else if arg == "-e" || arg == "--stderr" {
            stderr = true;
            idx += 1;
        } else if arg == "-o" || arg == "--stdout" {
            stdout = true;
            idx += 1;
        } else if arg == "-w" || arg == "--webhook" {
            if idx + 1 < raw.len() {
                webhook = Some(raw[idx + 1].clone());
                idx += 2;
            } else {
                idx += 1;
            }
        } else if arg == "-f" || arg == "--file" {
            if idx + 1 < raw.len() {
                file = Some(PathBuf::from(&raw[idx + 1]));
                idx += 2;
            } else {
                idx += 1;
            }
        } else if arg == "-g" || arg == "--grep" {
            if idx + 1 < raw.len() {
                grep = Some(raw[idx + 1].clone());
                idx += 2;
            } else {
                idx += 1;
            }
        } else {
            command = raw[idx..].to_vec();
            break;
        }
    }

    Config {
        webhook,
        file,
        stderr,
        stdout,
        grep,
        command,
        show_help,
        show_version,
        is_dl,
        dl_url,
    }
}

fn get_clipboard_url() -> Option<String> {
    // Try wl-paste (Wayland)
    if let Ok(out) = Command::new("wl-paste").output() {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.starts_with("http://") || text.starts_with("https://") {
            return Some(text);
        }
    }
    // Try xclip (X11)
    if let Ok(out) = Command::new("xclip").args(["-selection", "clipboard", "-o"]).output() {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.starts_with("http://") || text.starts_with("https://") {
            return Some(text);
        }
    }
    // Try xsel (X11)
    if let Ok(out) = Command::new("xsel").args(["--clipboard", "--output"]).output() {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.starts_with("http://") || text.starts_with("https://") {
            return Some(text);
        }
    }
    None
}

fn filter_by_grep(input: &[u8], pattern: &str) -> Vec<u8> {
    let text = String::from_utf8_lossy(input);
    let pattern_lower = pattern.to_lowercase();
    let filtered_lines: Vec<&str> = text
        .lines()
        .filter(|line| line.to_lowercase().contains(&pattern_lower))
        .collect();
    filtered_lines.join("\n").into_bytes()
}

fn resolve_webhook_url(explicit: Option<String>) -> Result<String> {
    if let Some(url) = explicit {
        if !url.trim().is_empty() {
            return Ok(url);
        }
    }

    if let Ok(url) = std::env::var("DISCORD_WEBHOOK_URL") {
        if !url.trim().is_empty() {
            return Ok(url);
        }
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config_paths = [
        format!("{}/.config/discord-log/webhook", home),
        format!("{}/.discord-log-webhook", home),
    ];

    for path in &config_paths {
        if let Ok(content) = fs::read_to_string(path) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }
    }

    bail!("Error: Discord Webhook URL is not set.\nRun: log --init <WEBHOOK_URL>");
}

async fn handle_download(url_opt: Option<String>) -> Result<()> {
    let client = reqwest::Client::new();

    let target_url = if let Some(url) = url_opt {
        url
    } else if let Some(clip_url) = get_clipboard_url() {
        println!("[log] Detected URL from clipboard: {}", clip_url);
        clip_url
    } else {
        bail!("No URL provided and clipboard does not contain a valid URL.\nUsage:\n  1) Copy link on Discord -> run: log dl\n  2) Pass URL directly: log dl <URL>");
    };

    let res = client.get(&target_url).send().await.context("Failed to download file")?;
    if !res.status().is_success() {
        bail!("Failed to download file from Discord (HTTP {})", res.status());
    }

    let fname = target_url
        .split('/')
        .last()
        .unwrap_or("downloaded_file")
        .split('?')
        .next()
        .unwrap_or("downloaded_file")
        .to_string();

    let bytes = res.bytes().await?;
    fs::write(&fname, bytes)?;
    println!("[log] Downloaded '{}' successfully!", fname);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let raw_args: Vec<String> = std::env::args().collect();
    if raw_args.len() >= 3 && raw_args[1] == "--init" {
        let url = &raw_args[2];
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let config_dir = format!("{}/.config/discord-log", home);
        fs::create_dir_all(&config_dir)?;
        let config_file = format!("{}/webhook", config_dir);
        fs::write(&config_file, url.trim())?;
        println!("Successfully saved Webhook URL to {}", config_file);
        return Ok(());
    }

    let args = parse_custom_args();

    if args.is_dl {
        return handle_download(args.dl_url).await;
    }

    if args.show_help {
        print!("{}", HELP_TEXT);
        return Ok(());
    }

    if args.show_version {
        println!("discord-log {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let webhook_url = resolve_webhook_url(args.webhook)?;

    let (buffer, filename, title, force_file) = if !args.command.is_empty() {
        let interrupted = Arc::new(AtomicBool::new(false));
        let interrupted_clone = interrupted.clone();

        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                interrupted_clone.store(true, Ordering::SeqCst);
            }
        });

        let cmd_name = &args.command[0];
        let cmd_args = &args.command[1..];

        let mut child = Command::new(cmd_name)
            .args(cmd_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to run command '{cmd_name}'"))?;

        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();

        let t_stdout = std::thread::spawn(move || {
            let mut out = io::stdout();
            let mut buf = [0u8; 1024];
            let mut res = Vec::new();
            while let Ok(n) = stdout.read(&mut buf) {
                if n == 0 { break; }
                let _ = out.write_all(&buf[..n]);
                let _ = out.flush();
                res.extend_from_slice(&buf[..n]);
            }
            res
        });

        let t_stderr = std::thread::spawn(move || {
            let mut err = io::stderr();
            let mut buf = [0u8; 1024];
            let mut res = Vec::new();
            while let Ok(n) = stderr.read(&mut buf) {
                if n == 0 { break; }
                let _ = err.write_all(&buf[..n]);
                let _ = err.flush();
                res.extend_from_slice(&buf[..n]);
            }
            res
        });

        let stdout_captured = t_stdout.join().unwrap_or_default();
        let stderr_captured = t_stderr.join().unwrap_or_default();

        let status = child.wait()?;
        let is_ctrl_c = interrupted.load(Ordering::SeqCst);

        let mut captured = Vec::new();
        if args.stderr {
            captured = stderr_captured;
        } else if args.stdout {
            captured = stdout_captured;
        } else {
            captured.extend_from_slice(&stdout_captured);
            if !stderr_captured.is_empty() {
                if !captured.is_empty() && !captured.ends_with(b"\n") {
                    captured.push(b'\n');
                }
                captured.extend_from_slice(&stderr_captured);
            }
        }

        if let Some(ref pattern) = args.grep {
            captured = filter_by_grep(&captured, pattern);
        }

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let fname = format!("{}_{}.log", cmd_name, timestamp);

        let status_str = if is_ctrl_c {
            "[INTERRUPTED]"
        } else if status.success() {
            "[SUCCESS]"
        } else {
            "[FAILED]"
        };
        let title_text = format!("{} `{}`", status_str, args.command.join(" "));

        (captured, fname, title_text, false)
    } else if let Some(ref file_path) = args.file {
        let mut bytes = fs::read(file_path).with_context(|| format!("Failed to read '{:?}'", file_path))?;
        let fname = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();

        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let is_binary_file = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "pdf" | "zip" | "tar" | "gz");

        if !is_binary_file {
            if let Some(ref pattern) = args.grep {
                bytes = filter_by_grep(&bytes, pattern);
            }
        }

        let title_text = format!("File: `{}`", fname);
        (bytes, fname, title_text, true)
    } else if !io::stdin().is_terminal() {
        let mut buffer = Vec::new();
        let mut stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut chunk = [0u8; 1024];

        while let Ok(n) = stdin.read(&mut chunk) {
            if n == 0 { break; }
            let _ = stdout.write_all(&chunk[..n]);
            let _ = stdout.flush();
            buffer.extend_from_slice(&chunk[..n]);
        }

        if let Some(ref pattern) = args.grep {
            buffer = filter_by_grep(&buffer, pattern);
        }

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let fname = format!("stream_{}.log", timestamp);
        let title_text = "Piped Stream Log".to_string();
        (buffer, fname, title_text, false)
    } else {
        print!("{}", HELP_TEXT);
        return Ok(());
    };

    if buffer.is_empty() {
        return Ok(());
    }

    upload_to_discord(&webhook_url, buffer, &filename, &title, force_file).await?;

    Ok(())
}

async fn upload_to_discord(
    webhook_url: &str,
    content: Vec<u8>,
    filename: &str,
    title: &str,
    force_file: bool,
) -> Result<()> {
    let client = reqwest::Client::new();
    let mut form = multipart::Form::new();

    let is_short_text = !force_file && content.len() <= 1500;
    let log_str = String::from_utf8_lossy(&content);

    let payload_content = if is_short_text {
        format!("{}\n```\n{}\n```", title, log_str.trim())
    } else {
        title.to_string()
    };

    let mut payload = serde_json::Map::new();
    payload.insert("content".to_string(), serde_json::Value::String(payload_content));

    let payload_json = serde_json::Value::Object(payload).to_string();
    form = form.text("payload_json", payload_json);

    if force_file || !is_short_text {
        let mime_type = match filename.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "pdf" => "application/pdf",
            "json" => "application/json",
            "txt" | "log" => "text/plain; charset=utf-8",
            _ => "application/octet-stream",
        };

        let part = multipart::Part::bytes(content)
            .file_name(filename.to_string())
            .mime_str(mime_type)?;
        form = form.part("files[0]", part);
    }

    let res = client.post(webhook_url).multipart(form).send().await?;

    if !res.status().is_success() {
        let status = res.status();
        let error_body = res.text().await.unwrap_or_default();
        bail!("Discord Webhook error (HTTP {}): {}", status, error_body);
    }

    Ok(())
}
