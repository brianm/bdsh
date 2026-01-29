use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "bdsh",
    about = "Better Distributed Shell - run commands on multiple hosts via SSH",
    version
)]
pub struct Cli {
    /// Watch an output directory instead of running commands
    #[arg(long, conflicts_with_all = ["host_source", "tag_filter", "command"])]
    pub watch: Option<PathBuf>,

    /// Host source: @file, @"cmd", inline (h1,h2), or omit for hosts file
    #[arg()]
    pub host_source: Option<String>,

    /// Tag filter: :tag, :t1:t2 (AND), :t1,:t2 (OR)
    #[arg()]
    pub tag_filter: Option<String>,

    /// Output directory (default: temp)
    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,

    /// Keep output directory on exit
    #[arg(short, long)]
    pub keep: bool,

    /// Disable watch window (window 0 with consensus view)
    #[arg(long)]
    pub no_watch: bool,

    /// Command to run on all hosts
    #[arg(last = true)]
    pub command: Vec<String>,
}
