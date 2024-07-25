use anyhow::{Context, Result};
use names::Generator;
use std::io::Write;
use std::process::{Command, Stdio};
use std::{thread, time};

fn main() -> Result<()> {
    let mut generator = Generator::default();
    let name = generator.next().unwrap();
    let mut control = Command::new("tmux")
        .args(["-C", "new-session", "-s", &name])
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .spawn()?;

    // TODO switch this to wartching for session readiness
    thread::sleep(time::Duration::from_secs(5));

    let mut ui_tmux = Command::new("tmux")
        .args(["attach", "-t", &name])
        .stdout(Stdio::inherit())
        .spawn()?;

    let mut control_stdin = control
        .stdin
        .take()
        .context("unable to take stdin for control process")?;

    control_stdin.write_all(b"new-window -d -n m0001 ssh m0001\n")?;
    control_stdin.write_all(b"new-window -d -n m0002 ssh m0002\n")?;

    ui_tmux.wait()?;
    control.kill()?;
    control.wait()?;
    Ok(())
}
