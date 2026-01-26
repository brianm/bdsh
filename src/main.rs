use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

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
    println!("{:#?}", cli);
    Ok(())
}

struct Job {
    /// Directory this job executes in
    root: PathBuf,

    /// hostname to run command on
    host: String,

    /// command to run
    command: String,
}
