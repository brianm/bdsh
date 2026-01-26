use anyhow::Result;
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

mod tmux;

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
            // Leak the tempdir so it doesn't get deleted when dropped
            // We'll handle cleanup ourselves
            tmp.into_path()
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

    // Start tmux control session with dedicated socket
    let mut control = tmux::Control::start_session_with_socket(
        &session_name,
        &socket_path,
        None,
    )?;

    // Create a window for each host
    for host in &hosts {
        let host_dir = output_dir.join(host);
        let log_file = host_dir.join("out.log");

        // Command: ssh $host $cmd | tee $log_file
        // Don't shell_escape here - new_window handles quoting for tmux
        let window_cmd = format!(
            "ssh {} {} 2>&1 | tee {}; echo 'Exit code:' $? >> {}",
            host,
            cmd_str,
            log_file.display(),
            log_file.display()
        );

        control.new_window(host, Some(&window_cmd))?;
    }

    // Attach to the tmux session for interactive use
    let mut ui_tmux = Command::new("tmux")
        .args(["-S", socket_path.to_str().unwrap(), "attach", "-t", &session_name])
        .spawn()?;

    ui_tmux.wait()?;
    control.kill()?;

    // Cleanup unless --keep
    if !cli.keep {
        println!("Cleaning up {}", output_dir.display());
        fs::remove_dir_all(&output_dir)?;
    } else {
        println!("Output preserved at {}", output_dir.display());
    }

    Ok(())
}

/// Simple shell escaping - wrap in single quotes, escape existing single quotes
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace("'", "'\\''"))
}
