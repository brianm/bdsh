use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "bdsh", about = "Better Distributed Shell")]
struct Cli {
    /// Host specification: inline (h1,h2) or @file/@executable
    host_spec: String,

    /// Filter hosts by column/field value
    #[arg(long = "where")]
    where_clause: Option<String>,

    /// Column number for hostname in tabular data (1-indexed)
    #[arg(long, default_value = "1")]
    host_col: usize,

    /// JSON pointer to hostname field in JSON/YAML objects
    #[arg(long)]
    host_ptr: Option<String>,

    /// Output directory (default: temp)
    #[arg(short, long)]
    output_dir: Option<PathBuf>,

    /// Keep output directory on exit
    #[arg(short, long)]
    keep: bool,

    /// Command to run on all hosts
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Parse hosts (just split on comma for now)
    let hosts: Vec<&str> = cli.host_spec.split(',').collect();

    // Create output directory
    let output_dir = match &cli.output_dir {
        Some(dir) => {
            fs::create_dir_all(dir)?;
            dir.clone()
        }
        None => {
            let tmp = tempfile::tempdir()?;
            tmp.keep()
        }
    };

    println!("Output directory: {}", output_dir.display());

    // Create host subdirectories
    for host in &hosts {
        fs::create_dir_all(output_dir.join(host))?;
    }

    // Build the command string
    let cmd_str = cli.command.join(" ");

    // Generate a session name
    let session_name = names::Generator::default().next().unwrap();

    // Socket path for isolation
    let socket_path = output_dir.join("tmux.sock");
    let socket_str = socket_path.to_str().unwrap();

    // Create tmux session with first host
    let mut hosts_iter = hosts.iter();
    let first_host = hosts_iter.next().context("Need at least one host")?;

    let first_cmd = format!("ssh -t {} {}", first_host, cmd_str);
    let first_log = output_dir.join(first_host).join("out.log");

    // Create detached session with first window (index 0)
    tmux(&socket_str, &[
        "new-session", "-d", "-s", &session_name, "-n", first_host, &first_cmd
    ])?;

    // Set up pipe-pane to capture output (use index, not name, to avoid dot parsing issues)
    tmux(&socket_str, &[
        "pipe-pane", "-t", &format!("{}:0", session_name),
        &format!("cat > {}", first_log.display())
    ])?;

    // Create windows for remaining hosts
    for (i, host) in hosts_iter.enumerate() {
        let host_cmd = format!("ssh -t {} {}", host, cmd_str);
        let host_log = output_dir.join(host).join("out.log");
        let window_index = i + 1; // First host is 0, so remaining start at 1

        tmux(&socket_str, &[
            "new-window", "-t", &session_name, "-n", host, &host_cmd
        ])?;

        tmux(&socket_str, &[
            "pipe-pane", "-t", &format!("{}:{}", session_name, window_index),
            &format!("cat > {}", host_log.display())
        ])?;
    }

    // Select first window
    tmux(&socket_str, &[
        "select-window", "-t", &format!("{}:0", session_name)
    ])?;

    // Attach to the tmux session for interactive use
    let status = Command::new("tmux")
        .args(["-S", socket_str, "attach", "-t", &session_name])
        .status()?;

    if !status.success() {
        eprintln!("tmux attach exited with: {}", status);
    }

    // Cleanup unless --keep
    if !cli.keep {
        println!("Cleaning up {}", output_dir.display());
        fs::remove_dir_all(&output_dir)?;
    } else {
        println!("Output preserved at {}", output_dir.display());
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
