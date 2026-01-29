mod consensus;
mod help_bar;
mod status_bar;

use crate::colors::ColorScheme;
use crate::Status;
use anyhow::Result;
use consensus::{
    clean_terminal_output, compute_consensus, format_gutter, max_gutter_width, ConsensusLine,
    ConsensusView, ConsensusViewWidget,
};
use help_bar::HelpBar;
use status_bar::StatusBar;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    tty::IsTty,
    ExecutableCommand,
};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Spacing},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::fs;
use std::io::{self, stdout, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Spinner frames for running status
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const SPINNER_INTERVAL_MS: u64 = 80;

/// Patterns that suggest a process is waiting for user input
const INPUT_PROMPT_PATTERNS: &[&str] = &[
    "password:",
    "passphrase",
    "[y/n]",
    "[Y/n]",
    "[n/Y]",
    "[yes/no]",
    "(yes/no)",
    "continue?",
    "proceed?",
    "confirm",
    "enter to continue",
    "press enter",
    "press any key",
    ": $",
    "? $",
    "> ",
    "read>",
];





/// Detect if output suggests the process is waiting for user input
fn detect_input_prompt(output: &str) -> bool {
    // Get last 500 chars to catch prompts without trailing newline
    // (lines() only returns complete lines, missing partial prompts)
    let tail: String = output
        .chars()
        .rev()
        .take(500)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let tail_lower = tail.to_lowercase();


    INPUT_PROMPT_PATTERNS
        .iter()
        .any(|pattern| tail_lower.contains(&pattern.to_lowercase()))
}

/// WatchApp - coordinator for the watch mode TUI
struct WatchApp {
    output_dir: PathBuf,

    // Components
    consensus_view: ConsensusView,
    color_scheme: ColorScheme,

    hosts: Vec<String>,
    statuses: HashMap<String, Status>,
    /// Cache of last-read outputs to detect changes
    last_outputs: HashMap<String, String>,
    /// Hosts that appear to be waiting for input
    waiting_for_input: HashMap<String, bool>,
    /// Whether output should be kept (creates .keep marker file)
    keep_output: bool,
    /// Spinner animation state
    spinner_frame: usize,
    spinner_last_update: Instant,
    /// Tail mode - auto-scroll to end
    tail_mode: bool,
}

impl WatchApp {
    fn new(output_dir: PathBuf) -> Self {
        // Check if .keep marker already exists
        let keep_output = output_dir.join(".keep").exists();
        Self {
            output_dir,
            consensus_view: ConsensusView::new(),
            color_scheme: ColorScheme::from_env(),
            hosts: Vec::new(),
            statuses: HashMap::new(),
            last_outputs: HashMap::new(),
            waiting_for_input: HashMap::new(),
            keep_output,
            spinner_frame: 0,
            spinner_last_update: Instant::now(),
            tail_mode: true,
        }
    }

    fn toggle_tail(&mut self) {
        self.tail_mode = !self.tail_mode;
        if self.tail_mode {
            // Jump to end when enabling tail mode
            self.consensus_view.scroll_to_end();
        }
    }

    /// Get the current spinner character and advance if needed
    fn spinner_char(&mut self) -> char {
        let now = Instant::now();
        if now.duration_since(self.spinner_last_update).as_millis() >= SPINNER_INTERVAL_MS as u128 {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
            self.spinner_last_update = now;
        }
        SPINNER_FRAMES[self.spinner_frame]
    }

    fn refresh(&mut self) -> Result<()> {
        self.hosts = discover_hosts(&self.output_dir)?;

        // Read statuses (always update these)
        self.statuses = self
            .hosts
            .iter()
            .map(|h| (h.clone(), read_status(&self.output_dir, h)))
            .collect();

        // Read outputs
        if !self.hosts.is_empty() {
            // Read raw outputs for prompt detection (before cleaning strips incomplete lines)
            let raw_outputs: HashMap<String, String> = self
                .hosts
                .iter()
                .map(|h| (h.clone(), read_raw_output(&self.output_dir, h)))
                .collect();

            // Detect hosts waiting for input (only for running hosts)
            self.waiting_for_input = raw_outputs
                .iter()
                .filter(|(h, _)| {
                    self.statuses.get(*h).copied() == Some(Status::Running)
                })
                .map(|(h, output)| (h.clone(), detect_input_prompt(output)))
                .filter(|(_, waiting)| *waiting)
                .collect();

            // Clean outputs for consensus display
            let outputs: HashMap<String, String> = raw_outputs
                .into_iter()
                .map(|(h, raw)| (h, clean_terminal_output(&raw)))
                .collect();

            // Only rebuild consensus if outputs changed
            if outputs != self.last_outputs {
                // Save expanded state by line index
                let expanded_indices: Vec<usize> = self
                    .consensus_view
                    .consensus
                    .iter()
                    .enumerate()
                    .filter_map(|(i, line)| match line {
                        ConsensusLine::Differs { expanded: true, .. } => Some(i),
                        _ => None,
                    })
                    .collect();

                // Rebuild consensus
                let outputs_ref: HashMap<&str, String> =
                    outputs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                let mut new_consensus = compute_consensus(&self.hosts, &outputs_ref);

                // Restore expanded state for indices that still exist and are diffs
                for i in expanded_indices {
                    if let Some(ConsensusLine::Differs { expanded, .. }) = new_consensus.get_mut(i)
                    {
                        *expanded = true;
                    }
                }

                self.consensus_view.update_consensus(new_consensus, true);
                self.last_outputs = outputs;
            }
        } else {
            self.consensus_view.update_consensus(Vec::new(), false);
            self.last_outputs.clear();
        }

        Ok(())
    }

    fn scroll_up(&mut self) {
        self.tail_mode = false; // Manual scroll disables tail
        self.consensus_view.scroll_up();
    }

    fn scroll_down(&mut self) {
        self.tail_mode = false; // Manual scroll disables tail
        self.consensus_view.scroll_down();
    }

    fn toggle_expand(&mut self) {
        self.consensus_view.toggle_expand();
    }

    fn expand_selected(&mut self) {
        self.consensus_view.expand_selected();
    }

    fn collapse_selected(&mut self) {
        self.consensus_view.collapse_selected();
    }

    fn expand_all(&mut self) {
        self.consensus_view.expand_all();
    }

    fn collapse_all(&mut self) {
        self.consensus_view.collapse_all();
    }

    fn toggle_keep(&mut self) {
        self.keep_output = !self.keep_output;
        let keep_marker = self.output_dir.join(".keep");
        if self.keep_output {
            // Create marker file
            let _ = fs::write(&keep_marker, "");
        } else {
            // Remove marker file
            let _ = fs::remove_file(&keep_marker);
        }
    }

    fn jump_to_next_diff(&mut self) {
        self.consensus_view.jump_to_next_diff();
    }
}

/// Run watch mode on an output directory
pub fn run(output_dir: &Path) -> Result<()> {
    // Check if we're running in a TTY - if not, fall back to text mode
    if !stdout().is_tty() {
        return run_text_mode(output_dir);
    }

    // Set up terminal for TUI
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui(&mut terminal, output_dir);

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

/// Run in text mode (for non-TTY environments like tests or piped output)
fn run_text_mode(output_dir: &Path) -> Result<()> {
    println!("Watching: {}", output_dir.display());

    let colors = ColorScheme::from_env();

    // Initial render
    let hosts = discover_hosts(output_dir)?;
    if hosts.is_empty() {
        println!("No host directories found yet...");
    } else {
        render_text_consensus(output_dir, &hosts, &colors)?;
    }

    // Set up channels for file events and stdin EOF
    enum TextEvent {
        FileChange,
        StdinClosed,
    }

    let (tx, rx) = mpsc::channel();

    // File watcher
    let file_tx = tx.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if res.is_ok() {
                let _ = file_tx.send(TextEvent::FileChange);
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(100)),
    )?;

    watcher.watch(output_dir, RecursiveMode::Recursive)?;

    // Stdin watcher - exits on EOF (Ctrl-D)
    let stdin_tx = tx;
    std::thread::spawn(move || {
        let mut stdin = io::stdin();
        let mut buf = [0u8; 1];
        // Block until EOF or error
        while stdin.read(&mut buf).unwrap_or(0) > 0 {}
        let _ = stdin_tx.send(TextEvent::StdinClosed);
    });

    // Debounce and re-render on changes
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(TextEvent::FileChange) => {
                // Drain any additional file events (debounce)
                while matches!(rx.try_recv(), Ok(TextEvent::FileChange)) {}

                // Re-render
                clear_screen();
                let hosts = discover_hosts(output_dir)?;
                if !hosts.is_empty() {
                    render_text_consensus(output_dir, &hosts, &colors)?;
                }
            }
            Ok(TextEvent::StdinClosed) => {
                // Ctrl-D pressed, exit cleanly
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No changes, continue watching
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

/// Clear the terminal screen (for text mode)
fn clear_screen() {
    print!("\x1B[2J\x1B[1;1H");
    io::stdout().flush().ok();
}

/// Render consensus view as plain text
fn render_text_consensus(output_dir: &Path, hosts: &[String], colors: &ColorScheme) -> Result<()> {
    if hosts.is_empty() {
        println!("No hosts found.");
        return Ok(());
    }

    // Read all outputs and statuses
    let outputs: HashMap<&str, String> = hosts
        .iter()
        .map(|h| (h.as_str(), read_output(output_dir, h)))
        .collect();

    let statuses: HashMap<&str, Status> = hosts
        .iter()
        .map(|h| (h.as_str(), read_status(output_dir, h)))
        .collect();

    // Header with status summary
    let status_summary: Vec<String> = hosts
        .iter()
        .map(|h| format!("{}:{}", h, format_status(statuses[h.as_str()], colors)))
        .collect();

    println!(
        "=== Consensus View ({} hosts) ===\n{}\n",
        hosts.len(),
        status_summary.join("  ")
    );

    // Compute and display consensus
    let consensus = compute_consensus(hosts, &outputs);

    for line in &consensus {
        match line {
            ConsensusLine::Identical(content) => {
                println!("{}", content);
            }
            ConsensusLine::Differs {
                consensus,
                variants,
                missing,
                ..
            } => {
                let variant_count = variants.len();
                // Show consensus with variant count indicator
                let formatted_marker = colors.ansi_yellow(&format!("[{}]", variant_count));
                println!("{} {}", formatted_marker, consensus);

                // Text mode never expands host lists, so pass None for expanded_hosts
                let max_width = max_gutter_width(variants, missing, None);

                // Show variants with host gutter on left
                for (content, hosts) in variants.iter() {
                    let gutter = format_gutter(hosts, false);
                    let formatted_gutter = colors.ansi_cyan(&format!("{:>width$}", gutter, width = max_width));
                    println!("  {} │ {}", formatted_gutter, content);
                }
                if !missing.is_empty() {
                    let gutter = format_gutter(missing, false);
                    let formatted_gutter = colors.ansi_cyan(&format!("{:>width$}", gutter, width = max_width));
                    let formatted_missing = colors.ansi_gray("<missing>");
                    println!("  {} │ {}", formatted_gutter, formatted_missing);
                }
            }
        }
    }

    Ok(())
}

/// Format status with ANSI color
fn format_status(status: Status, colors: &ColorScheme) -> String {
    let s = status.as_str();
    match status {
        Status::Running => colors.ansi_yellow(s),
        Status::Success => colors.ansi_green(s),
        Status::Failed => colors.ansi_red(s),
        Status::Pending => s.to_string(),
    }
}

fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    output_dir: &Path,
) -> Result<()> {
    let mut state = WatchApp::new(output_dir.to_path_buf());
    state.refresh()?; // Initial refresh
    if state.tail_mode {
        state.consensus_view.scroll_to_end();
    }

    loop {
        // Get spinner char (advances animation)
        let spinner = state.spinner_char();

        // Draw UI
        terminal.draw(|f| render_ui(f, &mut state, spinner))?;

        // Handle events with short timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match (key.code, key.modifiers) {
                        // Quit commands
                        (KeyCode::Char('q'), KeyModifiers::NONE)
                        | (KeyCode::Char('Q'), KeyModifiers::SHIFT)
                        | (KeyCode::Esc, _) => break,
                        (KeyCode::Char('d'), m) if m.contains(KeyModifiers::CONTROL) => break,
                        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => break,

                        // Navigation
                        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                            state.scroll_up()
                        }
                        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                            state.scroll_down()
                        }

                        // Expand/collapse with arrow keys (hierarchical)
                        (KeyCode::Right, _) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
                            state.expand_selected()
                        }
                        (KeyCode::Left, _) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
                            state.collapse_selected()
                        }

                        // Actions
                        (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => state.toggle_expand(),
                        (KeyCode::Tab, _) => state.jump_to_next_diff(),
                        (KeyCode::Char('e'), KeyModifiers::NONE) => state.expand_all(),
                        (KeyCode::Char('c'), KeyModifiers::NONE) => state.collapse_all(),
                        (KeyCode::Char('K'), KeyModifiers::SHIFT) => state.toggle_keep(),
                        (KeyCode::Char('t'), KeyModifiers::NONE) => state.toggle_tail(),

                        _ => {}
                    }
                }
            }
        }

        // Always refresh - reading small files is fast, and this avoids
        // any delays from file watcher event propagation
        let _ = state.refresh();
        if state.tail_mode {
            state.consensus_view.scroll_to_end();
        }
    }

    Ok(())
}

fn render_ui(f: &mut Frame, state: &mut WatchApp, spinner: char) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Status bar
            Constraint::Min(1),    // Main content
            Constraint::Length(3), // Help bar
        ])
        .spacing(Spacing::Overlap(1))
        .split(f.area());

    let status_bar = StatusBar::new(
        &state.hosts,
        &state.statuses,
        &state.waiting_for_input,
        spinner,
        state.spinner_frame,
        state.tail_mode,
        state.keep_output,
        &state.color_scheme,
    );
    f.render_widget(status_bar, chunks[0]);

    f.render_stateful_widget(ConsensusViewWidget::new(&state.color_scheme), chunks[1], &mut state.consensus_view);

    f.render_widget(HelpBar, chunks[2]);
}




/// Find all host subdirectories
fn discover_hosts(output_dir: &Path) -> Result<Vec<String>> {
    let mut hosts = Vec::new();

    if !output_dir.exists() {
        return Ok(hosts);
    }

    for entry in fs::read_dir(output_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name() {
                let name = name.to_string_lossy().to_string();
                // Skip tmux socket and other non-host entries
                if !name.starts_with('.') && name != "tmux.sock" {
                    hosts.push(name);
                }
            }
        }
    }

    hosts.sort();
    Ok(hosts)
}

/// Read raw output log for a host (for prompt detection)
fn read_raw_output(output_dir: &Path, host: &str) -> String {
    let log_path = output_dir.join(host).join("out.log");
    fs::read_to_string(&log_path).unwrap_or_default()
}

/// Read output log for a host (cleaned for display)
fn read_output(output_dir: &Path, host: &str) -> String {
    clean_terminal_output(&read_raw_output(output_dir, host))
}

/// Clean terminal output by processing carriage returns and stripping control chars

/// Read status for a host (running, success, failed, or pending)
fn read_status(output_dir: &Path, host: &str) -> Status {
    let status_path = output_dir.join(host).join("status");
    fs::read_to_string(&status_path)
        .map(|s| Status::from_str(&s))
        .unwrap_or(Status::Pending)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_consensus_identical() {
        let hosts = vec!["host1".to_string(), "host2".to_string()];
        let outputs: HashMap<&str, String> = [
            ("host1", "line1\nline2\nline3".to_string()),
            ("host2", "line1\nline2\nline3".to_string()),
        ]
        .into_iter()
        .collect();

        let consensus = compute_consensus(&hosts, &outputs);

        assert_eq!(consensus.len(), 3);
        assert!(matches!(&consensus[0], ConsensusLine::Identical(s) if s == "line1"));
        assert!(matches!(&consensus[1], ConsensusLine::Identical(s) if s == "line2"));
        assert!(matches!(&consensus[2], ConsensusLine::Identical(s) if s == "line3"));
    }

    #[test]
    fn test_compute_consensus_differs() {
        let hosts = vec![
            "host1".to_string(),
            "host2".to_string(),
            "host3".to_string(),
        ];
        let outputs: HashMap<&str, String> = [
            ("host1", "line1\nline2\nline3".to_string()),
            ("host2", "line1\nDIFFERENT\nline3".to_string()),
            ("host3", "line1\nline2\nline3".to_string()),
        ]
        .into_iter()
        .collect();

        let consensus = compute_consensus(&hosts, &outputs);

        // Simple line-by-line comparison: 3 lines
        // line1 (identical), line2 vs DIFFERENT (differs), line3 (identical)
        assert_eq!(consensus.len(), 3);
        assert!(matches!(&consensus[0], ConsensusLine::Identical(s) if s == "line1"));
        // line2 has "line2" (host1, host3) and "DIFFERENT" (host2)
        assert!(matches!(&consensus[1], ConsensusLine::Differs { variants, consensus, .. }
            if variants.contains_key("line2") && variants.contains_key("DIFFERENT") && consensus == "line2"));
        assert!(matches!(&consensus[2], ConsensusLine::Identical(s) if s == "line3"));
    }

    #[test]
    fn test_compute_consensus_single_host() {
        let hosts = vec!["host1".to_string()];
        let outputs: HashMap<&str, String> =
            [("host1", "line1\nline2".to_string())].into_iter().collect();

        let consensus = compute_consensus(&hosts, &outputs);

        assert_eq!(consensus.len(), 2);
        assert!(matches!(&consensus[0], ConsensusLine::Identical(s) if s == "line1"));
        assert!(matches!(&consensus[1], ConsensusLine::Identical(s) if s == "line2"));
    }

    #[test]
    fn test_compute_consensus_empty() {
        let hosts: Vec<String> = vec![];
        let outputs: HashMap<&str, String> = HashMap::new();

        let consensus = compute_consensus(&hosts, &outputs);
        assert!(consensus.is_empty());
    }

    #[test]
    fn test_detect_input_prompt_password() {
        assert!(detect_input_prompt("Connecting...\nPassword:"));
        assert!(detect_input_prompt("Enter your password:"));
        assert!(detect_input_prompt("SSH passphrase for key:"));
    }

    #[test]
    fn test_detect_input_prompt_confirmation() {
        assert!(detect_input_prompt("Proceed with installation? [y/n]"));
        assert!(detect_input_prompt("Continue? [Y/n]"));
        assert!(detect_input_prompt("Are you sure (yes/no)?"));
        assert!(detect_input_prompt("Do you want to continue?"));
    }

    #[test]
    fn test_detect_input_prompt_negative() {
        // Regular output shouldn't trigger
        assert!(!detect_input_prompt("Installing packages..."));
        assert!(!detect_input_prompt("Downloading file 1 of 10"));
        assert!(!detect_input_prompt("Build completed successfully"));
    }
}
