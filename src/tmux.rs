use std::io::Write;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::str::FromStr;
use thiserror::Error;

type Result<T> = std::result::Result<T, TmuxError>;

pub struct Control {
    name: String,
    tmux: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Control {
    pub fn start_session(name: &str, command: Option<String>) -> Result<Control> {
        let mut args = vec!["-C", "new-session", "-s", &name];
        let command: Option<&str> = command.as_deref();
        args.extend(command.iter());
        let mut tmux = Command::new("tmux")
            .args(args)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .map_err(TmuxError::IoError)?;

        let stdin = tmux.stdin.take().unwrap();
        let stdout = tmux.stdout.take().unwrap();

        let mut c = Control {
            name: name.into(),
            tmux,
            stdin,
            stdout: BufReader::new(stdout),
        };

        // now consume notifs until we see our session
        loop {
            let notif = c.consume_notification()?;
            match notif {
                Notification::SessionChanged(_, name) if name == c.name => break,
                _ => continue, //print!("skipping notif: {:?}\n", notif),
            }
        }
        Ok(c)
    }

    fn consume_notification(&mut self) -> Result<Notification> {
        let mut buf = String::new();
        self.stdout
            .read_line(&mut buf)
            .map_err(TmuxError::IoError)?;
        Ok(buf.parse()?)
    }

    pub fn kill(&mut self) -> Result<()> {
        self.tmux.kill().map_err(|err| -> TmuxError {
            TmuxError::ChildError {
                msg: format!("unable to kill {}", err),
                source: err,
            }
        })?;

        self.tmux.wait().map_err(|err| -> TmuxError {
            TmuxError::ChildError {
                msg: format!("unable to wait for child: {}", err),
                source: err,
            }
        })?;
        Ok(())
    }

    pub fn send(&mut self, command: &str) -> Result<()> {
        self.stdin
            .write_all(command.as_bytes())
            .map_err(TmuxError::IoError)?;
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum TmuxError {
    #[error("problem with communicating with child tmux: {0}")]
    IoError(#[from] std::io::Error),

    #[error("problem with child tmux: {msg}")]
    ChildError { msg: String, source: std::io::Error },

    #[error("notification parse error: {0}")]
    NotifParseError(String),
}

#[derive(Debug, PartialEq)]
enum Notification {
    SessionChanged(String, String),
    Other(String, Option<String>),
}

impl FromStr for Notification {
    type Err = TmuxError;

    fn from_str(data: &str) -> Result<Notification> {
        if data.is_empty() || !data.starts_with(r"%") {
            return Err(TmuxError::NotifParseError(
                "invalid input, not a notification string".into(),
            ));
        }
        let data = data.trim_end_matches("\n"); // strip trailing newline
        let (notif_type, notif_data) = match data.split_once(" ") {
            Some((notif_type, notif_data)) => (notif_type, Some(notif_data.into())),
            None => (data, None),
        };

        match notif_type {
            "%session-changed" => Notification::session_changed(notif_data),
            _ => Ok(Notification::Other(notif_type.into(), notif_data)),
        }
    }
}

impl Notification {
    fn session_changed(data: Option<String>) -> Result<Notification> {
        let data = data.ok_or_else(|| {
            TmuxError::NotifParseError("%session-changed notification missing data".into())
        })?;
        let (session_number, session_name) = match data.split_once(" ") {
            Some((session_number, session_name)) => (session_number, session_name),
            None => {
                return Err(TmuxError::NotifParseError(
                    "missing session name in %session-changed".into(),
                ))
            }
        };
        Ok(Notification::SessionChanged(
            session_number.into(),
            session_name.into(),
        ))
    }
}

mod test {
    use super::*;

    #[test]
    fn test_notification_parse() {
        let notif = "%session-changed 1 m0001\n"
            .parse::<Notification>()
            .unwrap();
        assert_eq!(
            notif,
            Notification::SessionChanged("1".into(), "m0001".into())
        );
    }
}
