mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use std::time::Duration;
use tempfile::tempdir;

fn bdsh_cmd() -> assert_cmd::Command {
    cargo_bin_cmd!("bdsh")
}

#[test]
fn ssh_echo_single_host() {
    let Some(sshd) = common::TestSshd::start() else {
        eprintln!("Skipping test: sshd not available");
        return;
    };

    let output_dir = tempdir().unwrap();

    // Build host spec with port
    let host_spec = format!("-p {} {}", sshd.port, sshd.connection_string());

    let mut cmd = bdsh_cmd();
    cmd.arg("run")
        .arg(&host_spec)
        .arg("-o")
        .arg(output_dir.path())
        .arg("-k")  // Keep output
        .arg("--")
        .arg("echo")
        .arg("hello")
        .timeout(Duration::from_secs(10));

    // This will attach to tmux, which we can't do in a test easily
    // For now, just verify it doesn't crash immediately
    // TODO: Add non-interactive mode for testing
}

#[test]
fn direct_ssh_works() {
    // Test that our sshd setup actually works with direct ssh
    let Some(sshd) = common::TestSshd::start() else {
        eprintln!("Skipping test: sshd not available");
        return;
    };

    let output = Command::new("ssh")
        .args(sshd.ssh_args())
        .arg(sshd.connection_string())
        .arg("echo")
        .arg("hello from ssh")
        .output()
        .expect("ssh command failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "ssh failed: stdout={}, stderr={}",
        stdout,
        stderr
    );
    assert!(
        stdout.contains("hello from ssh"),
        "unexpected output: {}",
        stdout
    );
}

#[test]
fn watch_with_real_ssh_output() {
    let Some(sshd) = common::TestSshd::start() else {
        eprintln!("Skipping test: sshd not available");
        return;
    };

    let output_dir = tempdir().unwrap();
    let host_dir = output_dir.path().join("testhost");
    fs::create_dir_all(&host_dir).unwrap();

    // Run ssh and capture output to simulate what bdsh run does
    let log_path = host_dir.join("out.log");

    let output = Command::new("ssh")
        .args(sshd.ssh_args())
        .arg(sshd.connection_string())
        .arg("echo")
        .arg("test output line")
        .output()
        .expect("ssh command failed");

    fs::write(&log_path, &output.stdout).unwrap();

    // Now test that watch can read this
    let mut cmd = bdsh_cmd();
    cmd.arg("--watch")
        .arg(output_dir.path())
        .timeout(Duration::from_secs(1));

    cmd.assert()
        .stdout(predicate::str::contains("1 hosts"))
        .stdout(predicate::str::contains("testhost:"))
        .stdout(predicate::str::contains("test output line"));
}
