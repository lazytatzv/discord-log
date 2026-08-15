use anyhow::{bail, Context, Result};
use clap::Parser;
use reqwest::multipart;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Stream/upload logs to Discord Webhooks cleanly with options for stdout/stderr filtering & Ctrl+C safety.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Target Discord Webhook URL (env: DISCORD_WEBHOOK_URL)
    #[arg(short, long, env = "DISCORD_WEBHOOK_URL")]
    webhook: Option<String>,

    /// Send an existing log file
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Title / Summary tag for the log
    #[arg(short, long)]
    title: Option<String>,

    /// Custom Bot name
    #[arg(short, long, default_value = "Log Bot")]
    username: String,

    /// Capture ONLY stderr (ignore stdout for Discord)
    #[arg(long)]
    err_only: bool,

    /// Capture ONLY stdout (ignore stderr for Discord)
    #[arg(long)]
    out_only: bool,

    /// Command to execute and capture (e.g. log colcon build)
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let webhook_url = match args.webhook {
        Some(url) if !url.trim().is_empty() => url,
        _ => bail!("Error: DISCORD_WEBHOOK_URL is not set.\nExport it in your shell or use --webhook <URL>."),
    };

    let (buffer, filename, default_title) = if !args.command.is_empty() {
        // Mode 1: Transparent Command Wrapper with Ctrl+C (SIGINT) Intercept
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
            .with_context(|| format!("Failed to spawn command '{cmd_name}'"))?;

        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();

        // Stream output to terminal live while capturing
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

        // Filter what to capture based on flags
        let mut captured = Vec::new();
        if args.err_only {
            captured = stderr_captured;
        } else if args.out_only {
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

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let fname = format!("{}_{}.log", cmd_name, timestamp);

        let status_str = if is_ctrl_c {
            "[INTERRUPTED]"
        } else if status.success() {
            "[SUCCESS]"
        } else {
            "[FAILED]"
        };
        let title = format!("{} `{}`", status_str, args.command.join(" "));

        (captured, fname, title)
    } else if let Some(ref file_path) = args.file {
        // Mode 2: Send log file
        let bytes = fs::read(file_path).with_context(|| format!("Failed to read '{:?}'", file_path))?;
        let fname = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("log.log").to_string();
        let title = format!("File: `{}`", fname);
        (bytes, fname, title)
    } else if !io::stdin().is_terminal() {
        // Mode 3: Stdin Pipe Wrapper
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

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let fname = format!("stream_{}.log", timestamp);
        let title = "Piped Stream Log".to_string();
        (buffer, fname, title)
    } else {
        bail!("No input provided.\n\nUsage Examples:\n  log colcon build\n  colcon build 2>&1 | log");
    };

    if buffer.is_empty() {
        return Ok(());
    }

    let final_title = args.title.unwrap_or(default_title);
    upload_to_discord(&webhook_url, buffer, &filename, &final_title, &args.username).await?;

    Ok(())
}

async fn upload_to_discord(
    webhook_url: &str,
    content: Vec<u8>,
    filename: &str,
    title: &str,
    username: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    let mut form = multipart::Form::new();

    let mut payload = serde_json::Map::new();
    if !title.is_empty() {
        payload.insert("content".to_string(), serde_json::Value::String(title.to_string()));
    }
    if !username.is_empty() {
        payload.insert("username".to_string(), serde_json::Value::String(username.to_string()));
    }

    if !payload.is_empty() {
        let payload_json = serde_json::Value::Object(payload).to_string();
        form = form.text("payload_json", payload_json);
    }

    let part = multipart::Part::bytes(content)
        .file_name(filename.to_string())
        .mime_str("text/plain; charset=utf-8")?;

    form = form.part("files[0]", part);

    let res = client.post(webhook_url).multipart(form).send().await?;

    if !res.status().is_success() {
        let status = res.status();
        let error_body = res.text().await.unwrap_or_default();
        bail!("Discord Webhook error (HTTP {}): {}", status, error_body);
    }

    Ok(())
}
