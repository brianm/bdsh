use anyhow::Result;
use names::Generator;
use std::env;
use std::path::PathBuf;
use std::process::{exit, Command};

mod tmux;

fn main() -> Result<()> {
    // TODO add clap to take various arguments

    // create a temp dir to work in, for now use argv[1]

    // start control tmux against socket in temp dir

    // fire up tmux instance in foreground in "watch this directory" mode

    // Run commands a la `ssh freki $command | tee $bdsh_tmp/$host/out.log`
    // so that we capture output and still get the nice tmux experience if input is needed

    //

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

struct Job {
    /// Directory this job executes in
    root: PathBuf,

    /// hostname to run command on
    host: String,

    /// command to run
    command: String,
}
