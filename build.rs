use clap::CommandFactory;
use std::fs;
use std::io::Write;
use std::path::Path;

#[path = "src/cli.rs"]
mod cli;

fn main() {
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=doc/bdsh-extra.md");

    let doc_dir = Path::new("doc");
    fs::create_dir_all(doc_dir).expect("Failed to create doc directory");

    // Generate base man page from clap
    let cmd = cli::Cli::command();
    let man = clap_mangen::Man::new(cmd);

    let man_path = doc_dir.join("bdsh.1");
    let mut file = fs::File::create(&man_path).expect("Failed to create man page file");
    man.render(&mut file).expect("Failed to render man page");

    // Convert markdown extras to roff and append
    let extra_md = fs::read_to_string("doc/bdsh-extra.md")
        .expect("Failed to read doc/bdsh-extra.md");
    let extra_roff = mandown::convert(&extra_md, "BDSH", 1);

    // Skip the .TH header line that mandown generates (we already have one from clap)
    let extra_roff = extra_roff
        .lines()
        .skip_while(|line| line.starts_with(".TH") || line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    file.write_all(b"\n").expect("Failed to write newline");
    file.write_all(extra_roff.as_bytes())
        .expect("Failed to write extra man sections");
}
