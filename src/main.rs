use anyhow::{bail, Context, Result};
use clap::Parser;
use reqwest::multipart;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(author, version, about = "Send command outputs to Discord Webhook")]
struct Args {
    /// Discord Webhook URL (env: DISCORD_WEBHOOK_URL)
    #[arg(short, long, env = "DISCORD_WEBHOOK_URL")]
    webhook: Option<String>,

    /// Log file to upload
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Send ONLY stderr
    #[arg(short, long)]
    stderr: bool,

    /// Send ONLY stdout
    #[arg(short, long)]
    stdout: bool,

    /// Command to execute (e.g. log colcon build)
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let webhook_url = match args.webhook {
        Some(url) if !url.trim().is_empty() => url,
        _ => bail!("Error: DISCORD_WEBHOOK_URL environment variable is not set."),
    };

    let (buffer, filename, title) = if !args.command.is_empty() {
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

        (captured, fname, title_text)
    } else if let Some(ref file_path) = args.file {
        let bytes = fs::read(file_path).with_context(|| format!("Failed to read '{:?}'", file_path))?;
        let fname = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("log.log").to_string();
        let title_text = format!("File: `{}`", fname);
        (bytes, fname, title_text)
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

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let fname = format!("stream_{}.log", timestamp);
        let title_text = "Piped Stream Log".to_string();
        (buffer, fname, title_text)
    } else {
        bail!("Usage:\n  log <command>\n  log -e <command>   # stderr only\n  log -o <command>   # stdout only");
    };

    if buffer.is_empty() {
        return Ok(());
    }

    upload_to_discord(&webhook_url, buffer, &filename, &title).await?;

    Ok(())
}

async fn upload_to_discord(
    webhook_url: &str,
    content: Vec<u8>,
    filename: &str,
    title: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    let mut form = multipart::Form::new();

    let mut payload = serde_json::Map::new();
    if !title.is_empty() {
        payload.insert("content".to_string(), serde_json::Value::String(title.to_string()));
    }

    let payload_json = serde_json::Value::Object(payload).to_string();
    form = form.text("payload_json", payload_json);

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
