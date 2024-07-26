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
        println!("sleeping for 30, C-c to terminate early");
        std::thread::sleep(std::time::Duration::from_secs(30));
        exit(0);
    }

    let name = Generator::default().next().unwrap();

    let mut control = tmux::Control::start_session(&name, Some(format!("{} {}", cmd, name)))?;

    let mut ui_tmux = Command::new("tmux").args(["attach", "-t", &name]).spawn()?;

    control.send("new-window -d -n m0001 sleep 3\n")?;
    control.send("new-window -d -n m0002 sleep 4\n")?;

    ui_tmux.wait()?;
    control.kill()?;
    println!("done");
    Ok(())
}
