use std::fs;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// A test SSH server that runs sshd on a random port
pub struct TestSshd {
    pub port: u16,
    pub host: String,
    _temp_dir: TempDir,
    process: Child,
}

impl TestSshd {
    /// Start a new sshd instance for testing.
    /// Returns None if sshd is not available or fails to start.
    pub fn start() -> Option<Self> {
        // Check if sshd is available by trying to run it with -?
        // This avoids depending on 'which' command which may not exist on all systems
        let sshd_check = Command::new("sshd")
            .arg("-?")
            .output();

        if sshd_check.is_err() {
            eprintln!("sshd not found, skipping SSH tests");
            return None;
        }

        // Find a free port
        let port = find_free_port()?;

        // Create temp directory for sshd files
        let temp_dir = TempDir::new().ok()?;
        let temp_path = temp_dir.path();

        // Generate host key
        let host_key_path = temp_path.join("host_key");
        let status = Command::new("ssh-keygen")
            .args([
                "-t", "ed25519",
                "-f", host_key_path.to_str()?,
                "-N", "",  // No passphrase
                "-q",      // Quiet
            ])
            .status()
            .ok()?;
        if !status.success() {
            eprintln!("Failed to generate host key");
            return None;
        }

        // Create authorized_keys from current user's public key
        let home = std::env::var("HOME").ok()?;
        let user_pubkey_path = PathBuf::from(&home).join(".ssh/id_ed25519.pub");
        let alt_pubkey_path = PathBuf::from(&home).join(".ssh/id_rsa.pub");

        let pubkey_path = if user_pubkey_path.exists() {
            user_pubkey_path
        } else if alt_pubkey_path.exists() {
            alt_pubkey_path
        } else {
            eprintln!("No SSH public key found (~/.ssh/id_ed25519.pub or id_rsa.pub)");
            return None;
        };

        let authorized_keys_path = temp_path.join("authorized_keys");
        fs::copy(&pubkey_path, &authorized_keys_path).ok()?;

        // Create sshd_config
        let config_path = temp_path.join("sshd_config");
        let pid_file = temp_path.join("sshd.pid");

        let config = format!(
            r#"
Port {port}
ListenAddress 127.0.0.1
HostKey {host_key}
PidFile {pid_file}

# Auth settings
PasswordAuthentication no
PubkeyAuthentication yes
AuthorizedKeysFile {authorized_keys}
StrictModes no

# Disable unnecessary features
UsePAM no
X11Forwarding no
PrintMotd no
PrintLastLog no

# Logging
LogLevel DEBUG
"#,
            port = port,
            host_key = host_key_path.display(),
            pid_file = pid_file.display(),
            authorized_keys = authorized_keys_path.display(),
        );
        fs::write(&config_path, config).ok()?;

        // Start sshd in debug mode (foreground, verbose)
        // Use "sshd" directly - the OS will find it in PATH
        let mut process = Command::new("sshd")
            .args([
                "-D",  // Don't daemonize
                "-e",  // Log to stderr
                "-f", config_path.to_str()?,
            ])
            .stderr(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .ok()?;

        // Wait for sshd to start by watching stderr for "Server listening"
        let stderr = process.stderr.take()?;
        let reader = BufReader::new(stderr);

        let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started_clone = started.clone();

        // Spawn thread to read stderr and detect startup
        thread::spawn(move || {
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.contains("Server listening") {
                        started_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    // Optionally print debug output
                    // eprintln!("[sshd] {}", line);
                }
            }
        });

        // Poll for startup with timeout
        for _ in 0..50 {
            if started.load(std::sync::atomic::Ordering::SeqCst) {
                return Some(TestSshd {
                    port,
                    host: "127.0.0.1".to_string(),
                    _temp_dir: temp_dir,
                    process,
                });
            }
            thread::sleep(Duration::from_millis(100));
        }

        eprintln!("sshd failed to start within timeout");
        let _ = process.kill();
        None
    }

    /// Get SSH connection string (user@host)
    pub fn connection_string(&self) -> String {
        let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
        format!("{}@{}", user, self.host)
    }

    /// Get SSH command args for connecting to this server
    pub fn ssh_args(&self) -> Vec<String> {
        vec![
            "-o".to_string(), "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(), "UserKnownHostsFile=/dev/null".to_string(),
            "-o".to_string(), "LogLevel=ERROR".to_string(),
            "-p".to_string(), self.port.to_string(),
        ]
    }
}

impl Drop for TestSshd {
    fn drop(&mut self) {
        // Send SIGTERM to sshd
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// Find a free TCP port by binding to port 0
fn find_free_port() -> Option<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").ok()?;
    let port = listener.local_addr().ok()?.port();
    drop(listener);
    Some(port)
}
