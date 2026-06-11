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
use help_bar::{HelpBar, HelpContext};
use status_bar::StatusBar;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    tty::IsTty,
    ExecutableCommand,
};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect, Spacing},
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
/// Patterns that match anywhere in the tail of the output
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
    "--more--",
    "--More--",
    "(END)",
    "read>",
];

/// Patterns that only match at the very end of the tail (prompt characters).
/// Checked against the tail after stripping trailing whitespace/newlines.
const INPUT_PROMPT_SUFFIX_PATTERNS: &[&str] = &[
    ":",
    "> ",
    "? ",
    "$ ",
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

    // Specific multi-char phrases are unambiguous — check anywhere in the tail
    if INPUT_PROMPT_PATTERNS
        .iter()
        .any(|pattern| tail_lower.contains(&pattern.to_lowercase()))
    {
        return true;
    }

    // Short prompt characters are only meaningful at the end of output.
    // Strip trailing whitespace/newlines before checking, since prompts
    // often lack a trailing newline but may have trailing spaces.
    let tail_trimmed = tail_lower.trim_end_matches(['\n', '\r', ' ']);
    INPUT_PROMPT_SUFFIX_PATTERNS
        .iter()
        .any(|pattern| tail_trimmed.ends_with(&pattern.to_lowercase()))
}

/// The number shown for `hosts[0]` in the status bar. 1-based so the index
/// matches the tmux window number (window 0 is the watch view itself).
pub(super) const WINDOW_BASE: usize = 1;

/// Map a user-typed window number to a host. Returns `None` for `0` or any
/// number outside the host list. Shared by the status-bar prefix and the
/// log-view selection so the two can't drift apart.
fn host_for_window_number(hosts: &[String], typed: usize) -> Option<&str> {
    typed
        .checked_sub(WINDOW_BASE)
        .and_then(|i| hosts.get(i))
        .map(String::as_str)
}

/// Which view is active in the main content area.
enum ViewMode {
    /// Default unified consensus/diff view.
    Consensus,
    /// User is typing a host index number to open its log.
    NumberEntry { buffer: String },
    /// Full-screen scrollable log of a single host.
    /// `tail` keeps the view pinned to the bottom as new output arrives.
    Log {
        host: String,
        scroll: usize,
        tail: bool,
    },
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
    /// Which view is active in the main content area
    mode: ViewMode,
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
            mode: ViewMode::Consensus,
        }
    }

    /// Enter host-index entry mode (triggered by `v`). No-op if there are no hosts.
    fn start_number_entry(&mut self) {
        if !self.hosts.is_empty() {
            self.mode = ViewMode::NumberEntry {
                buffer: String::new(),
            };
        }
    }

    /// Resolve a key press while in NumberEntry mode into the next view mode.
    /// Pure with respect to terminal/filesystem so it can be unit-tested.
    fn handle_number_entry_key(&self, buffer: &str, key: KeyCode) -> ViewMode {
        match key {
            KeyCode::Char(c) if c.is_ascii_digit() => ViewMode::NumberEntry {
                buffer: format!("{}{}", buffer, c),
            },
            KeyCode::Backspace => {
                let mut buffer = buffer.to_string();
                buffer.pop();
                ViewMode::NumberEntry { buffer }
            }
            KeyCode::Enter => match buffer.parse::<usize>() {
                Ok(n) => match host_for_window_number(&self.hosts, n) {
                    Some(host) => ViewMode::Log {
                        host: host.to_string(),
                        scroll: 0,
                        tail: true,
                    },
                    None => ViewMode::Consensus,
                },
                Err(_) => ViewMode::Consensus,
            },
            KeyCode::Esc => ViewMode::Consensus,
            // Ignore any other key: stay in entry mode with the buffer unchanged.
            _ => ViewMode::NumberEntry {
                buffer: buffer.to_string(),
            },
        }
    }

    /// Dispatch a key press based on the active view mode. Returns `true` if the
    /// app should quit. Kept free of terminal/event-loop concerns so it can be
    /// driven directly from tests.
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Ctrl-C / Ctrl-D quit from any mode.
        if matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return true;
        }

        if matches!(self.mode, ViewMode::NumberEntry { .. }) {
            // Resolve the keypress against the current buffer.
            let buffer = match &self.mode {
                ViewMode::NumberEntry { buffer } => buffer.clone(),
                _ => unreachable!(),
            };
            self.mode = self.handle_number_entry_key(&buffer, key.code);
        } else if matches!(self.mode, ViewMode::Log { .. }) {
            match (key.code, key.modifiers) {
                (KeyCode::Char('q'), KeyModifiers::NONE)
                | (KeyCode::Char('Q'), KeyModifiers::SHIFT)
                | (KeyCode::Esc, _) => self.mode = ViewMode::Consensus,
                (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                    if let ViewMode::Log { scroll, tail, .. } = &mut self.mode {
                        // Manual scroll up stops following the tail.
                        *tail = false;
                        *scroll = scroll.saturating_sub(1);
                    }
                }
                (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                    if let ViewMode::Log { scroll, .. } = &mut self.mode {
                        *scroll += 1;
                    }
                }
                (KeyCode::Home, _) | (KeyCode::Char('g'), KeyModifiers::NONE) => {
                    if let ViewMode::Log { scroll, tail, .. } = &mut self.mode {
                        *tail = false;
                        *scroll = 0;
                    }
                }
                (KeyCode::End, _) | (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
                    if let ViewMode::Log { tail, .. } = &mut self.mode {
                        // Jump to bottom and resume following.
                        *tail = true;
                    }
                }
                (KeyCode::Char('t'), KeyModifiers::NONE) => {
                    if let ViewMode::Log { tail, .. } = &mut self.mode {
                        *tail = !*tail;
                    }
                }
                _ => {}
            }
        } else {
            // Consensus mode
            match (key.code, key.modifiers) {
                // Quit commands
                (KeyCode::Char('q'), KeyModifiers::NONE)
                | (KeyCode::Char('Q'), KeyModifiers::SHIFT)
                | (KeyCode::Esc, _) => return true,

                // Navigation
                (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => self.scroll_up(),
                (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => self.scroll_down(),

                // Expand/collapse with arrow keys (hierarchical)
                (KeyCode::Right, _) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
                    self.expand_selected()
                }
                (KeyCode::Left, _) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
                    self.collapse_selected()
                }

                // Actions
                (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => self.toggle_expand(),
                (KeyCode::Tab, _) => self.jump_to_next_diff(),
                (KeyCode::Char('e'), KeyModifiers::NONE) => self.expand_all(),
                (KeyCode::Char('c'), KeyModifiers::NONE) => self.collapse_all(),
                (KeyCode::Char('K'), KeyModifiers::SHIFT) => self.toggle_keep(),
                (KeyCode::Char('t'), KeyModifiers::NONE) => self.toggle_tail(),
                (KeyCode::Char('v'), KeyModifiers::NONE) => self.start_number_entry(),

                _ => {}
            }
        }

        false
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
                if key.kind == KeyEventKind::Press && state.handle_key(key) {
                    break;
                }
            }
        }

        // Always refresh - reading small files is fast, and this avoids
        // any delays from file watcher event propagation
        let _ = state.refresh();
        if matches!(state.mode, ViewMode::Consensus) && state.tail_mode {
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

    let mut status_bar = StatusBar::new(
        &state.hosts,
        &state.statuses,
        &state.waiting_for_input,
        spinner,
        state.spinner_frame,
        state.tail_mode,
        state.keep_output,
        &state.color_scheme,
    );
    if matches!(state.mode, ViewMode::Log { .. }) {
        status_bar.view_label = "Log View";
    }
    f.render_widget(status_bar, chunks[0]);

    // Main content + help depend on the active mode. The status bar above
    // renders in every mode so the index numbers stay visible.
    match &mut state.mode {
        ViewMode::Consensus => {
            f.render_stateful_widget(
                ConsensusViewWidget::new(&state.color_scheme),
                chunks[1],
                &mut state.consensus_view,
            );
            f.render_widget(HelpBar::new(HelpContext::Consensus), chunks[2]);
        }
        ViewMode::NumberEntry { buffer } => {
            // Keep the consensus view visible underneath for context.
            f.render_stateful_widget(
                ConsensusViewWidget::new(&state.color_scheme),
                chunks[1],
                &mut state.consensus_view,
            );
            f.render_widget(HelpBar::new(HelpContext::NumberEntry(buffer)), chunks[2]);
        }
        ViewMode::Log { host, scroll, tail } => {
            render_log_view(f, chunks[1], host, scroll, *tail, &state.output_dir, &state.hosts);
            f.render_widget(HelpBar::new(HelpContext::Log { tail: *tail }), chunks[2]);
        }
    }
}

/// Render a single host's full (cleaned) log, scrollable. Re-reads the log each
/// frame so a still-running host's output updates live. When `tail` is set, the
/// view is pinned to the bottom; `scroll` is synced to the bottom so toggling
/// tail off leaves the view where it is. Otherwise `scroll` is clamped to range.
fn render_log_view(
    f: &mut Frame,
    area: Rect,
    host: &str,
    scroll: &mut usize,
    tail: bool,
    output_dir: &Path,
    hosts: &[String],
) {
    let text = if hosts.iter().any(|h| h == host) {
        read_output(output_dir, host)
    } else {
        "(host no longer present)".to_string()
    };

    // Viewport = area minus borders, matching ConsensusViewWidget's inner-height.
    let inner_height = area.height.saturating_sub(2) as usize;
    let max_scroll = text.lines().count().saturating_sub(inner_height);
    // Tail pins to the bottom; otherwise just keep scroll within range.
    if tail || *scroll > max_scroll {
        *scroll = max_scroll;
    }

    let paragraph = ratatui::widgets::Paragraph::new(text)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title(format!("Log: {} (q/Esc to return)", host))
                .merge_borders(ratatui::symbols::merge::MergeStrategy::Exact),
        )
        .scroll((*scroll as u16, 0));
    f.render_widget(paragraph, area);
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
    use ratatui::backend::TestBackend;
    use tempfile::tempdir;

    /// Create a stub host directory with output and status, the same layout the
    /// tmux sessions write — so the TUI can be driven without any real SSH.
    fn stub_host(dir: &Path, host: &str, out: &str, status: &str) {
        let host_dir = dir.join(host);
        fs::create_dir_all(&host_dir).unwrap();
        fs::write(host_dir.join("out.log"), out).unwrap();
        fs::write(host_dir.join("status"), status).unwrap();
    }

    /// Render the app into an in-memory TestBackend and flatten the buffer to a
    /// single string for substring assertions.
    fn render_to_string(state: &mut WatchApp, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| render_ui(f, state, '*')).unwrap();
        let buf = terminal.backend().buffer();
        let area = buf.area;
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
            out.push('\n');
        }
        out
    }

    fn press(state: &mut WatchApp, code: KeyCode) -> bool {
        state.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn test_status_bar_shows_window_index_prefix() {
        let dir = tempdir().unwrap();
        stub_host(dir.path(), "alpha", "out\n", "success");
        stub_host(dir.path(), "beta", "out\n", "running");

        let mut state = WatchApp::new(dir.path().to_path_buf());
        state.refresh().unwrap();
        let screen = render_to_string(&mut state, 80, 24);

        // Hosts are sorted: alpha -> [1], beta -> [2] (matches tmux window numbers).
        assert!(screen.contains("[1]alpha"), "missing alpha prefix:\n{screen}");
        assert!(screen.contains("[2]beta"), "missing beta prefix:\n{screen}");
    }

    #[test]
    fn test_v_opens_selected_host_log() {
        let dir = tempdir().unwrap();
        stub_host(dir.path(), "alpha", "ALPHA_ONLY_LINE\n", "success");
        stub_host(dir.path(), "beta", "BETA_ONLY_LINE\n", "success");

        let mut state = WatchApp::new(dir.path().to_path_buf());
        state.refresh().unwrap();

        // v -> number-entry, type "2" (beta), Enter -> log view for beta.
        assert!(!press(&mut state, KeyCode::Char('v')));
        assert!(matches!(state.mode, ViewMode::NumberEntry { .. }));
        assert!(!press(&mut state, KeyCode::Char('2')));
        assert!(!press(&mut state, KeyCode::Enter));
        assert!(
            matches!(&state.mode, ViewMode::Log { host, tail, .. } if host == "beta" && *tail),
            "expected tailing log view for beta, got {:?}",
            mode_name(&state.mode)
        );

        let screen = render_to_string(&mut state, 80, 24);
        assert!(screen.contains("Log: beta"), "missing log title:\n{screen}");
        assert!(screen.contains("BETA_ONLY_LINE"), "missing beta log:\n{screen}");
        // The other host's output must not leak into beta's log view.
        assert!(!screen.contains("ALPHA_ONLY_LINE"), "alpha leaked:\n{screen}");
        // The status bar reflects the active mode.
        assert!(screen.contains("Log View"), "status bar not relabeled:\n{screen}");
        assert!(!screen.contains("Consensus View"), "stale consensus label:\n{screen}");
    }

    #[test]
    fn test_log_view_tail_toggle_and_scroll() {
        let dir = tempdir().unwrap();
        stub_host(dir.path(), "alpha", "l1\nl2\nl3\n", "success");

        let mut state = WatchApp::new(dir.path().to_path_buf());
        state.refresh().unwrap();

        // The status bar always shows its own [TAIL] (consensus tail_mode), so
        // count occurrences: the log help bar adds a second one while tailing.
        let tail_count = |s: &str| s.matches("[TAIL]").count();

        // Open alpha's log (window 1). Tail is on by default.
        press(&mut state, KeyCode::Char('v'));
        press(&mut state, KeyCode::Char('1'));
        press(&mut state, KeyCode::Enter);
        assert!(matches!(state.mode, ViewMode::Log { tail: true, .. }));
        assert_eq!(tail_count(&render_to_string(&mut state, 80, 24)), 2);

        // `t` toggles tail off — the help bar's indicator goes away.
        press(&mut state, KeyCode::Char('t'));
        assert!(matches!(state.mode, ViewMode::Log { tail: false, .. }));
        assert_eq!(tail_count(&render_to_string(&mut state, 80, 24)), 1);

        // SHIFT-`G` jumps to the bottom and resumes tailing.
        state.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert!(matches!(state.mode, ViewMode::Log { tail: true, .. }));

        // Scrolling up stops tailing again.
        press(&mut state, KeyCode::Up);
        assert!(matches!(state.mode, ViewMode::Log { tail: false, .. }));

        // q returns to the consensus view.
        press(&mut state, KeyCode::Char('q'));
        assert!(matches!(state.mode, ViewMode::Consensus));
    }

    /// Small helper for assertion messages.
    fn mode_name(mode: &ViewMode) -> &'static str {
        match mode {
            ViewMode::Consensus => "Consensus",
            ViewMode::NumberEntry { .. } => "NumberEntry",
            ViewMode::Log { .. } => "Log",
        }
    }

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
    fn test_host_for_window_number() {
        let hosts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        // 1-based: [1] -> first host, [3] -> last host
        assert_eq!(host_for_window_number(&hosts, 1), Some("a"));
        assert_eq!(host_for_window_number(&hosts, 3), Some("c"));
        // 0 maps to nothing (window 0 is the watch view)
        assert_eq!(host_for_window_number(&hosts, 0), None);
        // Beyond the host count
        assert_eq!(host_for_window_number(&hosts, 4), None);
        // Empty host list
        assert_eq!(host_for_window_number(&[], 1), None);
    }

    #[test]
    fn test_handle_number_entry_key() {
        let mut app = WatchApp::new(PathBuf::from("/tmp/does-not-matter"));
        app.hosts = vec!["a".to_string(), "b".to_string()];

        // Digit appends to the buffer
        let mode = app.handle_number_entry_key("", KeyCode::Char('2'));
        assert!(matches!(&mode, ViewMode::NumberEntry { buffer } if buffer == "2"));

        // Backspace pops
        let mode = app.handle_number_entry_key("12", KeyCode::Backspace);
        assert!(matches!(&mode, ViewMode::NumberEntry { buffer } if buffer == "1"));

        // Enter on a valid index opens that host's log, tailing by default
        let mode = app.handle_number_entry_key("2", KeyCode::Enter);
        assert!(matches!(&mode, ViewMode::Log { host, scroll, tail }
            if host == "b" && *scroll == 0 && *tail));

        // Enter on an out-of-range / empty / zero index returns to consensus
        assert!(matches!(
            app.handle_number_entry_key("9", KeyCode::Enter),
            ViewMode::Consensus
        ));
        assert!(matches!(
            app.handle_number_entry_key("", KeyCode::Enter),
            ViewMode::Consensus
        ));
        assert!(matches!(
            app.handle_number_entry_key("0", KeyCode::Enter),
            ViewMode::Consensus
        ));

        // Esc cancels
        assert!(matches!(
            app.handle_number_entry_key("1", KeyCode::Esc),
            ViewMode::Consensus
        ));
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
        // Progress indicators with % should not trigger
        assert!(!detect_input_prompt("Downloading: 50%\n"));
        assert!(!detect_input_prompt("Progress: 100%\n"));
        assert!(!detect_input_prompt("[=====>   ] 75%\n"));
        // Colons in regular output should not trigger
        assert!(!detect_input_prompt("Step 1: build\nStep 2: test\n"));
        assert!(!detect_input_prompt("2024-01-01T12:00:00Z something happened\n"));
    }

    #[test]
    fn test_detect_input_prompt_pager() {
        // less/more pager prompts at end of output should trigger
        assert!(detect_input_prompt("some output\n:"));
        assert!(detect_input_prompt("some output\n--More--"));
        assert!(detect_input_prompt("some output\n(END)"));
    }
}
