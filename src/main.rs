use anyhow::Result;
use names::Generator;
use std::env;
use std::process::{exit, Command};

mod tmux;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let cmd = args.first().unwrap();
    if args.len() == 2 {
        // invoked from self inside tmux
        println!("sleeping for 10, C-c to terminate early");
        std::thread::sleep(std::time::Duration::from_secs(10));
        exit(0);
    }

    let name = Generator::default().next().unwrap();

    let mut control = tmux::Control::start_session(&name, Some(format!("{} {}", cmd, name)))?;

    let mut ui_tmux = Command::new("tmux").args(["attach", "-t", &name]).spawn()?;

    dbg!(control.new_window("m0001", Some("sleep 4"))?);
    dbg!(control.new_window("m0002", Some("sleep 4"))?);
    dbg!(control.new_window("m0003", Some("sleep 4"))?);
    dbg!(control.new_window("m0004", Some("sleep 4"))?);
    dbg!(control.new_window("m0005", Some("sleep 4"))?);
    dbg!(control.new_window("m0006", Some("sleep 4"))?);

    ui_tmux.wait()?;
    control.kill()?;
    println!("done");
    Ok(())
}
