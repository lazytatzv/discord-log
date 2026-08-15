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
  log dl [N | URL]   Download latest log (log dl), N-th previous log (log dl 1, log dl 2), or URL

Options:
  --init <URL>       Save Discord Webhook URL (one-time setup)
  --init-token <T>   Save Discord Bot Token (enables fetching history from Discord Channel)
  -c, --comment <msg> Add a comment/message to the log output
  -e, --stderr       Send ONLY stderr output
  -o, --stdout       Send ONLY stdout output
  -g, --grep <word>  Filter output lines matching keyword (case-insensitive)
  -f, --file <path>  Upload file directly as attachment
  -w, --webhook <URL> Override Discord Webhook URL
  -h, --help         Show this help message
  -V, --version      Show version

Examples:
  log -c "Raspberry Pi Kernel Build" colcon build
  log dl             # Downloads the latest log
  log dl 1           # Downloads the 2nd latest log (1 step back)
  log dl 2           # Downloads the 3rd latest log (2 steps back)
"#;

struct Config {
    webhook: Option<String>,
    file: Option<PathBuf>,
    comment: Option<String>,
    stderr: bool,
    stdout: bool,
    grep: Option<String>,
    command: Vec<String>,
    show_help: bool,
    show_version: bool,
    is_dl: bool,
    dl_arg: Option<String>,
}

fn parse_custom_args() -> Config {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut webhook = None;
    let mut file = None;
    let mut comment = None;
    let mut stderr = false;
    let mut stdout = false;
    let mut grep = None;
    let mut command = Vec::new();
    let mut show_help = false;
    let mut show_version = false;
    let mut is_dl = false;
    let mut dl_arg = None;

    if !raw.is_empty() && raw[0] == "dl" {
        is_dl = true;
        if raw.len() > 1 && !raw[1].starts_with('-') {
            dl_arg = Some(raw[1].clone());
        }
        return Config {
            webhook,
            file,
            comment,
            stderr,
            stdout,
            grep,
            command,
            show_help,
            show_version,
            is_dl,
            dl_arg,
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
        } else if arg == "-c" || arg == "--comment" || arg == "-m" || arg == "--message" {
            if idx + 1 < raw.len() {
                comment = Some(raw[idx + 1].clone());
                idx += 2;
            } else {
                idx += 1;
            }
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
        comment,
        stderr,
        stdout,
        grep,
        command,
        show_help,
        show_version,
        is_dl,
        dl_arg,
    }
}

fn get_cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(format!("{}/.cache/discord-log/history", home));
    let _ = fs::create_dir_all(&dir);
    dir
}

fn save_to_history(filename: &str, content: &[u8]) {
    let dir = get_cache_dir();
    let file_path = dir.join(filename);
    let _ = fs::write(&file_path, content);
}

fn get_history_files() -> Vec<PathBuf> {
    let dir = get_cache_dir();
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort_by(|a, b| {
        let ma = fs::metadata(a).and_then(|m| m.modified()).ok();
        let mb = fs::metadata(b).and_then(|m| m.modified()).ok();
        mb.cmp(&ma)
    });
    files
}

fn get_clipboard_url() -> Option<String> {
    if let Ok(out) = Command::new("wl-paste").output() {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.starts_with("http://") || text.starts_with("https://") {
            return Some(text);
        }
    }
    if let Ok(out) = Command::new("xclip").args(["-selection", "clipboard", "-o"]).output() {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if text.starts_with("http://") || text.starts_with("https://") {
            return Some(text);
        }
    }
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

fn get_bot_token() -> Option<String> {
    if let Ok(t) = std::env::var("DISCORD_BOT_TOKEN") {
        if !t.trim().is_empty() {
            return Some(t.trim().to_string());
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let path = format!("{}/.config/discord-log/token", home);
    if let Ok(content) = fs::read_to_string(path) {
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

async fn handle_download(dl_arg: Option<String>, webhook_url_opt: Option<String>) -> Result<()> {
    let client = reqwest::Client::new();

    // Mode A: Direct URL argument
    if let Some(ref arg) = dl_arg {
        if arg.starts_with("http://") || arg.starts_with("https://") {
            let res = client.get(arg).send().await.context("Failed to download file from URL")?;
            if !res.status().is_success() {
                bail!("Failed to download file from Discord (HTTP {})", res.status());
            }
            let fname = arg
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
            return Ok(());
        }
    }

    // Mode B: Discord API channel history (if --init-token is set)
    if let Some(bot_token) = get_bot_token() {
        let webhook_url = resolve_webhook_url(webhook_url_opt)?;
        let res = client.get(&webhook_url).send().await.context("Failed to fetch Webhook info")?;
        if res.status().is_success() {
            let webhook_json: serde_json::Value = res.json().await?;
            if let Some(channel_id) = webhook_json["channel_id"].as_str() {
                let limit = 50;
                let messages_url = format!("https://discord.com/api/v10/channels/{}/messages?limit={}", channel_id, limit);
                let msg_res = client
                    .get(&messages_url)
                    .header("Authorization", format!("Bot {}", bot_token))
                    .send()
                    .await;

                if let Ok(resp) = msg_res {
                    if resp.status().is_success() {
                        let msgs: Vec<serde_json::Value> = resp.json().await?;
                        let mut attachments = Vec::new();
                        for m in msgs {
                            if let Some(atts) = m["attachments"].as_array() {
                                for a in atts {
                                    if let (Some(url), Some(filename)) = (a["url"].as_str(), a["filename"].as_str()) {
                                        attachments.push((filename.to_string(), url.to_string()));
                                    }
                                }
                            }
                        }

                        if !attachments.is_empty() {
                            let idx = if let Some(ref arg) = dl_arg {
                                arg.parse::<usize>().unwrap_or(0)
                            } else {
                                0
                            };

                            if idx >= attachments.len() {
                                bail!("Index out of range. Found {} recent attachments on Discord channel.", attachments.len());
                            }

                            let (fname, target_url) = &attachments[idx];
                            let dl_bytes = client.get(target_url).send().await?.bytes().await?;
                            fs::write(fname, dl_bytes)?;
                            println!("[log] Downloaded '{}' from Discord Channel (step {})", fname, idx);
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    // Mode C: Fallback to local execution history (Zero-config fallback)
    let history = get_history_files();
    if !history.is_empty() {
        let idx = if let Some(ref arg) = dl_arg {
            arg.parse::<usize>().unwrap_or(0)
        } else {
            0
        };

        if idx < history.len() {
            let target_path = &history[idx];
            let fname = target_path.file_name().and_then(|n| n.to_str()).unwrap_or("log.log");
            let content = fs::read(target_path)?;
            fs::write(fname, content)?;
            println!("[log] Downloaded '{}' from history (step {})", fname, idx);
            return Ok(());
        }
    }

    // Mode D: Clipboard fallback
    if let Some(clip_url) = get_clipboard_url() {
        println!("[log] Detected URL from clipboard: {}", clip_url);
        let res = client.get(&clip_url).send().await.context("Failed to download file")?;
        if !res.status().is_success() {
            bail!("Failed to download file (HTTP {})", res.status());
        }
        let fname = clip_url
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
        return Ok(());
    }

    bail!("Could not download log.\nSetup options:\n  1) Set Bot Token for Discord API: log --init-token <BOT_TOKEN>\n  2) Copy Discord attachment link -> log dl\n  3) Pass URL directly: log dl <URL>");
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

    if raw_args.len() >= 3 && raw_args[1] == "--init-token" {
        let token = &raw_args[2];
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let config_dir = format!("{}/.config/discord-log", home);
        fs::create_dir_all(&config_dir)?;
        let config_file = format!("{}/token", config_dir);
        fs::write(&config_file, token.trim())?;
        println!("Successfully saved Bot Token to {}", config_file);
        return Ok(());
    }

    let args = parse_custom_args();

    if args.is_dl {
        return handle_download(args.dl_arg, args.webhook).await;
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

    save_to_history(&filename, &buffer);

    upload_to_discord(&webhook_url, buffer, &filename, &title, args.comment.as_deref(), force_file).await?;

    Ok(())
}

async fn upload_to_discord(
    webhook_url: &str,
    content: Vec<u8>,
    filename: &str,
    title: &str,
    comment: Option<&str>,
    force_file: bool,
) -> Result<()> {
    let client = reqwest::Client::new();
    let mut form = multipart::Form::new();

    let is_short_text = !force_file && content.len() <= 1500;
    let log_str = String::from_utf8_lossy(&content);

    let comment_prefix = if let Some(c) = comment {
        format!("💬 **{}**\n", c)
    } else {
        "".to_string()
    };

    let payload_content = if is_short_text {
        format!("{}{}\n```\n{}\n```", comment_prefix, title, log_str.trim())
    } else {
        format!("{}{}", comment_prefix, title)
    };

    let mut payload = serde_json::Map::new();
    payload.insert("content".to_string(), serde_json::Value::String(payload_content));

    let payload_json = serde_json::Value::Object(payload).to_string();
    form = form.text("payload_json", payload_json);

    if force_file || !is_short_text {
        let mime_type = match filename.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/gif",
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
