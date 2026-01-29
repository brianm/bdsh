use clap::CommandFactory;
use std::fs;
use std::path::Path;

#[path = "../src/cli.rs"]
mod cli;

/// Generate the man page from clap and markdown sources
pub fn generate_man_page() -> String {
    // Generate base man page from clap
    let cmd = cli::Cli::command();
    let man = clap_mangen::Man::new(cmd);

    let mut buffer = Vec::new();
    man.render(&mut buffer).expect("Failed to render man page");

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

    // Combine base and extras
    let mut result = String::from_utf8(buffer).expect("Invalid UTF-8 in man page");
    result.push('\n');
    result.push_str(&extra_roff);
    result.push('\n');

    result
}

/// Test that the checked-in man page matches what would be generated
#[test]
fn test_man_page_up_to_date() {
    let generated = generate_man_page();
    let checked_in = fs::read_to_string("doc/bdsh.1")
        .expect("Failed to read doc/bdsh.1");

    if generated != checked_in {
        eprintln!("ERROR: doc/bdsh.1 is out of date!");
        eprintln!("Run: cargo test --test generate_man -- --nocapture --ignored");
        eprintln!("Then commit the updated doc/bdsh.1");

        // Write to a temp file for easy comparison
        let temp_path = "/tmp/bdsh.1.generated";
        fs::write(temp_path, &generated).expect("Failed to write temp file");
        eprintln!("\nGenerated version written to: {}", temp_path);
        eprintln!("Compare with: diff doc/bdsh.1 {}", temp_path);

        panic!("Man page is out of date");
    }
}

/// Ignored test that actually regenerates the man page
/// Run with: cargo test --test generate_man -- --nocapture --ignored
#[test]
#[ignore]
fn regenerate_man_page() {
    let man_content = generate_man_page();
    let man_path = Path::new("doc/bdsh.1");

    fs::write(man_path, man_content)
        .expect("Failed to write doc/bdsh.1");

    println!("✓ Generated doc/bdsh.1");
    println!("Please commit the updated man page.");
}
