use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

mod cli;
mod hosts;
mod watch;

pub use cli::Cli;

/// Status of a command running on a host
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pending,
    Running,
    Success,
    Failed,
}

impl Status {
    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            "running" => Self::Running,
            "success" => Self::Success,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(watch_dir) = cli.watch {
        watch::run(&watch_dir)
    } else {
        // Parse host_source and tag_filter from positional args
        // Pattern: [source] [tag_filter] -- command
        // - If first arg starts with : it's a tag filter (source is config)
        // - If first arg starts with @ it's a source, second may be tag filter
        // - Otherwise first arg is inline hosts
        let (source, filter) = parse_host_args(cli.host_source.as_deref(), cli.tag_filter.as_deref());

        if cli.command.is_empty() {
            anyhow::bail!(
                "Command required: bdsh [source] [filter] -- command\n\
                 Or use: bdsh --watch <output-dir> to view existing output"
            );
        }

        run_command(source, filter, cli.output_dir, cli.keep, cli.no_watch, &cli.command)
    }
}

/// Parse positional args into (source, filter) tuple
fn parse_host_args<'a>(arg1: Option<&'a str>, arg2: Option<&'a str>) -> (Option<&'a str>, Option<&'a str>) {
    match (arg1, arg2) {
        // No args - use config, no filter
        (None, None) => (None, None),

        // One arg starting with : - config with filter
        (Some(a1), None) if a1.starts_with(':') => (None, Some(a1)),

        // One arg - source, no filter
        (Some(a1), None) => (Some(a1), None),

        // Two args - source and filter
        (Some(a1), Some(a2)) => (Some(a1), Some(a2)),

        // Shouldn't happen with clap
        (None, Some(_)) => (None, None),
    }
}

fn run_command(
    source: Option<&str>,
    filter: Option<&str>,
    output_dir: Option<PathBuf>,
    keep: bool,
    no_watch: bool,
    command: &[String],
) -> Result<()> {
    // Resolve hosts from source with optional filter
    let mut hosts = hosts::resolve_hosts(source, filter)?;
    hosts.sort(); // Sort for consistent window numbering with watch view

    // Create output directory
    let output_dir = match output_dir {
        Some(dir) => {
            fs::create_dir_all(&dir)?;
            dir
        }
        None => {
            let tmp = tempfile::tempdir()?;
            tmp.keep()
        }
    };

    // Create host subdirectories
    for host in &hosts {
        fs::create_dir_all(output_dir.join(host))?;
    }

    // Build the command string
    let cmd_str = command.join(" ");

    // Generate a session name
    let session_name = names::Generator::default()
        .next()
        .context("Failed to generate session name")?;

    // Socket path for isolation
    let socket_path = output_dir.join("tmux.sock");
    let socket_str = socket_path
        .to_str()
        .context("Output directory path contains invalid UTF-8")?;

    // Get path to this executable for watch window
    let exe_path = std::env::current_exe()?;
    let exe_str = exe_path.to_str().context("Invalid executable path")?;

    // Window index offset: 0 if no_watch (hosts start at 0), 1 if watch (hosts start at 1)
    let host_window_offset: usize = if no_watch { 0 } else { 1 };

    // Create detached session
    if no_watch {
        // First window is first host command
        let first_host = hosts.first().context("Need at least one host")?;
        let first_host_dir = output_dir.join(first_host);
        let first_script = generate_command_script(&first_host_dir, first_host, &cmd_str)?;
        let first_cmd = format!("sh {}", first_script.display());

        tmux(socket_str, &[
            "new-session", "-d", "-s", &session_name, "-n", first_host, &first_cmd,
        ])?;
    } else {
        // First window (0) is watch mode
        let watch_cmd = format!("{} --watch {}", exe_str, output_dir.display());
        tmux(socket_str, &[
            "new-session", "-d", "-s", &session_name, "-n", "watch", &watch_cmd,
        ])?;
    }

    // Create host command windows
    for (i, host) in hosts.iter().enumerate() {
        let host_dir = output_dir.join(host);
        let host_script = generate_command_script(&host_dir, host, &cmd_str)?;
        let host_log = host_dir.join("out.log");
        let window_index = i + host_window_offset;

        let host_cmd = format!("sh {}", host_script.display());

        // First host already created if no_watch, otherwise create new window
        if no_watch && i == 0 {
            // Already created as session's first window
        } else {
            tmux(socket_str, &[
                "new-window", "-t", &session_name, "-n", host, &host_cmd,
            ])?;
        }

        // Set up pipe-pane to capture output
        // Use dd with no buffering to capture partial lines (like prompts without newlines)
        tmux(socket_str, &[
            "pipe-pane",
            "-t",
            &format!("{}:{}", session_name, window_index),
            &format!("dd bs=1 of={} 2>/dev/null", host_log.display()),
        ])?;
    }

    // Select watch window (0) if available, otherwise first host window
    tmux(socket_str, &[
        "select-window",
        "-t",
        &format!("{}:0", session_name),
    ])?;

    // Attach to the tmux session for interactive use
    let status = Command::new("tmux")
        .args(["-S", socket_str, "attach", "-t", &session_name])
        .status()?;

    if !status.success() {
        eprintln!("tmux attach exited with: {}", status);
    }

    // Cleanup unless --keep or .keep marker exists (set by 'K' in watch TUI)
    let keep_marker = output_dir.join(".keep");
    if !keep && !keep_marker.exists() {
        fs::remove_dir_all(&output_dir)?;
    } else {
        // Remove the marker file but keep the rest
        let _ = fs::remove_file(&keep_marker);
        println!("{}", output_dir.display());
    }

    Ok(())
}

/// Run a tmux command with the given socket
fn tmux(socket: &str, args: &[&str]) -> Result<()> {
    let status = Command::new("tmux")
        .args(["-S", socket])
        .args(args)
        .status()
        .with_context(|| format!("Failed to run tmux {:?}", args))?;

    if !status.success() {
        anyhow::bail!("tmux {:?} failed: {}", args, status);
    }
    Ok(())
}

/// Generate a command wrapper script for a host
fn generate_command_script(host_dir: &Path, host: &str, command: &str) -> Result<PathBuf> {
    let script_path = host_dir.join("command");
    let status_path = host_dir.join("status");
    let meta_path = host_dir.join("meta.json");

    // Shell-escape the command for embedding in the script
    let escaped_command = command.replace("'", "'\\''");

    let script = format!(
        r#"#!/bin/sh
# bdsh command wrapper for {host}
# Note: No set -e here - we need to capture the exit code from ssh

STATUS_FILE="{status_path}"
META_FILE="{meta_path}"

echo "running" > "$STATUS_FILE"
START=$(date +%s.%N)

ssh -t {host} '{escaped_command}'
EXIT_CODE=$?

END=$(date +%s.%N)

cat > "$META_FILE" << METAEOF
{{"exit_code": $EXIT_CODE, "start": $START, "end": $END}}
METAEOF

if [ $EXIT_CODE -eq 0 ]; then
  echo "success" > "$STATUS_FILE"
else
  echo "failed" > "$STATUS_FILE"
fi

exit $EXIT_CODE
"#,
        host = host,
        escaped_command = escaped_command,
        status_path = status_path.display(),
        meta_path = meta_path.display(),
    );

    fs::write(&script_path, &script)?;

    // Make executable
    let mut perms = fs::metadata(&script_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms)?;

    Ok(script_path)
}
