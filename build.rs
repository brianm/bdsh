use clap::CommandFactory;
use std::fs;
use std::io::Write;
use std::path::Path;

#[path = "src/cli.rs"]
mod cli;

fn main() {
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=doc/bdsh-extra.md");

    // Get OUT_DIR for generated files (required by cargo publish)
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    // Generate base man page from clap
    let cmd = cli::Cli::command();
    let man = clap_mangen::Man::new(cmd);

    let man_path = out_path.join("bdsh.1");
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

    // Also update doc/bdsh.1 if we're in the actual source tree (not during cargo publish)
    // This keeps the committed copy up to date during development
    let doc_dir = Path::new("doc");
    if doc_dir.exists() && std::env::var("CARGO_PRIMARY_PACKAGE").is_ok() {
        let _ = fs::copy(&man_path, doc_dir.join("bdsh.1"));
    }
}
