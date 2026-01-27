use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn bdsh_cmd() -> Command {
    cargo_bin_cmd!("bdsh")
}

#[test]
fn watch_shows_identical_output() {
    let dir = tempdir().unwrap();
    let output_dir = dir.path();

    // Create host directories with identical output
    for host in &["host1", "host2", "host3"] {
        let host_dir = output_dir.join(host);
        fs::create_dir_all(&host_dir).unwrap();
        fs::write(host_dir.join("out.log"), "same output\n").unwrap();
    }

    // Run watch with timeout (it would run forever otherwise)
    let mut cmd = bdsh_cmd();
    cmd.arg("--watch")
        .arg(output_dir)
        .timeout(std::time::Duration::from_secs(1));

    cmd.assert()
        .stdout(predicate::str::contains("3 hosts"))
        .stdout(predicate::str::contains("same output"));
}

#[test]
fn watch_shows_differences() {
    let dir = tempdir().unwrap();
    let output_dir = dir.path();

    // Create host directories with different output
    let host1_dir = output_dir.join("host1");
    let host2_dir = output_dir.join("host2");
    fs::create_dir_all(&host1_dir).unwrap();
    fs::create_dir_all(&host2_dir).unwrap();

    fs::write(host1_dir.join("out.log"), "line one\n").unwrap();
    fs::write(host2_dir.join("out.log"), "line two\n").unwrap();

    let mut cmd = bdsh_cmd();
    cmd.arg("--watch")
        .arg(output_dir)
        .timeout(std::time::Duration::from_secs(1));

    cmd.assert()
        .stdout(predicate::str::contains("variants"))
        .stdout(predicate::str::contains("host1"))
        .stdout(predicate::str::contains("host2"));
}

#[test]
fn watch_handles_empty_output() {
    let dir = tempdir().unwrap();
    let output_dir = dir.path();

    // Create host directories with no output yet
    for host in &["host1", "host2"] {
        let host_dir = output_dir.join(host);
        fs::create_dir_all(&host_dir).unwrap();
        // Create empty log file
        fs::write(host_dir.join("out.log"), "").unwrap();
    }

    let mut cmd = bdsh_cmd();
    cmd.arg("--watch")
        .arg(output_dir)
        .timeout(std::time::Duration::from_secs(1));

    // With empty files, we just show the hosts header
    cmd.assert()
        .stdout(predicate::str::contains("2 hosts"));
}

#[test]
fn watch_ignores_tmux_socket() {
    let dir = tempdir().unwrap();
    let output_dir = dir.path();

    // Create a host and a tmux socket file
    let host_dir = output_dir.join("host1");
    fs::create_dir_all(&host_dir).unwrap();
    fs::write(host_dir.join("out.log"), "output\n").unwrap();

    // Create tmux.sock (should be ignored)
    fs::write(output_dir.join("tmux.sock"), "socket").unwrap();

    let mut cmd = bdsh_cmd();
    cmd.arg("--watch")
        .arg(output_dir)
        .timeout(std::time::Duration::from_secs(1));

    cmd.assert()
        .stdout(predicate::str::contains("1 hosts"))
        .stdout(predicate::str::contains("host1:"));
}

#[test]
fn requires_command() {
    let mut cmd = bdsh_cmd();
    // No command - should fail (host_source is optional, uses config)
    cmd.arg("--").assert().failure();
}
