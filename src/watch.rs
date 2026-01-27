use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    tty::IsTty,
    ExecutableCommand,
};
use indexmap::IndexMap;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
// Note: similar crate still available if we need diff-based view later
use std::collections::HashMap;
use std::fs;
use std::io::{self, stdout, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A line in the consensus view
#[derive(Clone, Debug)]
enum ConsensusLine {
    /// All hosts have identical content
    Identical(String),
    /// Hosts have different content - show most common, allow expansion
    Differs {
        /// The most common content (shown by default)
        consensus: String,
        /// How many hosts have the consensus version
        consensus_count: usize,
        /// Total number of hosts
        total_hosts: usize,
        /// All variants: content -> list of hosts (ordered by frequency, most common first)
        variants: IndexMap<String, Vec<String>>,
        /// Hosts missing this line entirely
        missing: Vec<String>,
        /// Currently expanded to show all variants?
        expanded: bool,
        /// Which variant contents have their host lists expanded (for [N] groups)
        expanded_hosts: std::collections::HashSet<String>,
    },
}

/// Spinner frames for running status
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const SPINNER_INTERVAL_MS: u64 = 80;

/// State for the watch mode TUI
struct WatchState {
    output_dir: PathBuf,
    hosts: Vec<String>,
    statuses: HashMap<String, String>,
    consensus: Vec<ConsensusLine>,
    selected_line: usize,
    /// Which variant is selected within an expanded Differs (None = the main line)
    selected_variant: Option<usize>,
    /// Cache of last-read outputs to detect changes
    last_outputs: HashMap<String, String>,
    /// Whether output should be kept (creates .keep marker file)
    keep_output: bool,
    /// Spinner animation state
    spinner_frame: usize,
    spinner_last_update: Instant,
    /// Tail mode - auto-scroll to end
    tail_mode: bool,
}

impl WatchState {
    fn new(output_dir: PathBuf) -> Self {
        // Check if .keep marker already exists
        let keep_output = output_dir.join(".keep").exists();
        Self {
            output_dir,
            hosts: Vec::new(),
            statuses: HashMap::new(),
            consensus: Vec::new(),
            selected_line: 0,
            selected_variant: None,
            last_outputs: HashMap::new(),
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
            self.scroll_to_end();
        }
    }

    fn scroll_to_end(&mut self) {
        if !self.consensus.is_empty() {
            self.selected_line = self.consensus.len() - 1;
            // If last line is expanded Differs, select last variant
            if let Some(ConsensusLine::Differs {
                expanded: true,
                variants,
                missing,
                ..
            }) = self.consensus.get(self.selected_line)
            {
                let variant_count = variants.len() + if missing.is_empty() { 0 } else { 1 };
                if variant_count > 0 {
                    self.selected_variant = Some(variant_count - 1);
                }
            } else {
                self.selected_variant = None;
            }
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
            let outputs: HashMap<String, String> = self
                .hosts
                .iter()
                .map(|h| (h.clone(), read_output(&self.output_dir, h)))
                .collect();

            // Only rebuild consensus if outputs changed
            if outputs != self.last_outputs {
                // Save expanded state by line index
                let expanded_indices: Vec<usize> = self
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
                self.consensus = compute_consensus(&self.hosts, &outputs_ref);

                // Restore expanded state for indices that still exist and are diffs
                for i in expanded_indices {
                    if let Some(ConsensusLine::Differs { expanded, .. }) =
                        self.consensus.get_mut(i)
                    {
                        *expanded = true;
                    }
                }

                self.last_outputs = outputs;
            }
        } else {
            self.consensus.clear();
            self.last_outputs.clear();
        }

        // Adjust scroll/selection if needed
        let max_line = self.consensus.len().saturating_sub(1);
        if self.selected_line > max_line {
            self.selected_line = max_line;
        }

        Ok(())
    }

    fn scroll_up(&mut self) {
        self.tail_mode = false; // Manual scroll disables tail

        // If we're in a variant, try to move up within variants
        if let Some(var_idx) = self.selected_variant {
            if var_idx > 0 {
                self.selected_variant = Some(var_idx - 1);
                return;
            } else {
                // At first variant, exit to main line
                self.selected_variant = None;
                return;
            }
        }

        // Move to previous consensus line
        if self.selected_line > 0 {
            self.selected_line -= 1;
            // If previous line is expanded Differs, select its last variant
            if let Some(ConsensusLine::Differs {
                expanded: true,
                variants,
                missing,
                ..
            }) = self.consensus.get(self.selected_line)
            {
                let variant_count = variants.len() + if missing.is_empty() { 0 } else { 1 };
                if variant_count > 0 {
                    self.selected_variant = Some(variant_count - 1);
                }
            }
        }
    }

    fn scroll_down(&mut self) {
        self.tail_mode = false; // Manual scroll disables tail

        // Check if current line is an expanded Differs
        if let Some(ConsensusLine::Differs {
            expanded: true,
            variants,
            missing,
            ..
        }) = self.consensus.get(self.selected_line)
        {
            let variant_count = variants.len() + if missing.is_empty() { 0 } else { 1 };

            if let Some(var_idx) = self.selected_variant {
                if var_idx + 1 < variant_count {
                    // Move to next variant
                    self.selected_variant = Some(var_idx + 1);
                    return;
                } else {
                    // At last variant, move to next consensus line
                    self.selected_variant = None;
                    if self.selected_line < self.consensus.len().saturating_sub(1) {
                        self.selected_line += 1;
                    }
                    return;
                }
            } else {
                // On main line of expanded Differs, enter variants
                if variant_count > 0 {
                    self.selected_variant = Some(0);
                    return;
                }
            }
        }

        // Move to next consensus line
        self.selected_variant = None;
        if self.selected_line < self.consensus.len().saturating_sub(1) {
            self.selected_line += 1;
        }
    }

    fn toggle_expand(&mut self) {
        if let Some(ConsensusLine::Differs { expanded, .. }) =
            self.consensus.get_mut(self.selected_line)
        {
            *expanded = !*expanded;
        }
    }

    /// Expand the selected line/variant (right-arrow behavior)
    /// - If on collapsed Differs main line: expand to show variants
    /// - If on a variant with [N] hosts: expand that variant's host list
    fn expand_selected(&mut self) {
        if let Some(ConsensusLine::Differs {
            expanded,
            variants,
            missing,
            expanded_hosts,
            ..
        }) = self.consensus.get_mut(self.selected_line)
        {
            if !*expanded {
                // Expand to show variants
                *expanded = true;
            } else if let Some(var_idx) = self.selected_variant {
                // Expand the selected variant's host list
                let variant_count = variants.len();
                if var_idx < variant_count {
                    // It's a regular variant
                    if let Some((content, hosts)) = variants.get_index(var_idx) {
                        if hosts.len() > 1 {
                            expanded_hosts.insert(content.clone());
                        }
                    }
                } else if var_idx == variant_count && !missing.is_empty() {
                    // It's the missing line
                    if missing.len() > 1 {
                        expanded_hosts.insert("<missing>".to_string());
                    }
                }
            }
        }
    }

    /// Collapse the selected line/variant (left-arrow behavior)
    /// - If on a variant with expanded hosts: collapse that variant
    /// - If on main line with expanded variants: collapse all
    fn collapse_selected(&mut self) {
        if let Some(ConsensusLine::Differs {
            expanded,
            variants,
            expanded_hosts,
            ..
        }) = self.consensus.get_mut(self.selected_line)
        {
            if let Some(var_idx) = self.selected_variant {
                // Try to collapse this variant's host list
                let variant_count = variants.len();
                let key = if var_idx < variant_count {
                    variants.get_index(var_idx).map(|(k, _)| k.clone())
                } else {
                    Some("<missing>".to_string())
                };

                if let Some(k) = key {
                    if expanded_hosts.remove(&k) {
                        return; // Collapsed a host list
                    }
                }
                // No host list to collapse, move selection up
                if var_idx > 0 {
                    self.selected_variant = Some(var_idx - 1);
                } else {
                    self.selected_variant = None;
                }
            } else if *expanded {
                // Collapse the variants
                *expanded = false;
                expanded_hosts.clear();
            }
        }
    }

    fn expand_all(&mut self) {
        for line in &mut self.consensus {
            if let ConsensusLine::Differs { expanded, .. } = line {
                *expanded = true;
            }
        }
    }

    fn collapse_all(&mut self) {
        for line in &mut self.consensus {
            if let ConsensusLine::Differs { expanded, .. } = line {
                *expanded = false;
            }
        }
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
        let start = self.selected_line + 1;
        for i in start..self.consensus.len() {
            if matches!(self.consensus[i], ConsensusLine::Differs { .. }) {
                self.selected_line = i;
                return;
            }
        }
        // Wrap around
        for i in 0..start.min(self.consensus.len()) {
            if matches!(self.consensus[i], ConsensusLine::Differs { .. }) {
                self.selected_line = i;
                return;
            }
        }
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

    // Initial render
    let hosts = discover_hosts(output_dir)?;
    if hosts.is_empty() {
        println!("No host directories found yet...");
    } else {
        render_text_consensus(output_dir, &hosts)?;
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
                    render_text_consensus(output_dir, &hosts)?;
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
fn render_text_consensus(output_dir: &Path, hosts: &[String]) -> Result<()> {
    if hosts.is_empty() {
        println!("No hosts found.");
        return Ok(());
    }

    // Read all outputs and statuses
    let outputs: HashMap<&str, String> = hosts
        .iter()
        .map(|h| (h.as_str(), read_output(output_dir, h)))
        .collect();

    let statuses: HashMap<&str, String> = hosts
        .iter()
        .map(|h| (h.as_str(), read_status(output_dir, h)))
        .collect();

    // Header with status summary
    let status_summary: Vec<String> = hosts
        .iter()
        .map(|h| format!("{}:{}", h, format_status(&statuses[h.as_str()])))
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
                consensus_count,
                total_hosts,
                variants,
                missing,
                ..
            } => {
                let diff_count = total_hosts - consensus_count;
                // Show consensus with diff indicator
                println!("\x1b[33m[{}]\x1b[0m {}", diff_count, consensus);

                // Calculate max gutter width for alignment
                let max_gutter_width = variants
                    .iter()
                    .map(|(_, hosts)| {
                        if hosts.len() == 1 {
                            hosts[0].len()
                        } else {
                            format!("[{}]", hosts.len()).len()
                        }
                    })
                    .chain(if missing.is_empty() {
                        None
                    } else if missing.len() == 1 {
                        Some(missing[0].len())
                    } else {
                        Some(format!("[{}]", missing.len()).len())
                    })
                    .max()
                    .unwrap_or(4)
                    .max(4);

                // Show variants with host gutter on left
                for (content, hosts) in variants.iter() {
                    let host_count = hosts.len();
                    let gutter = if host_count == 1 {
                        hosts[0].clone()
                    } else {
                        format!("[{}]", host_count)
                    };
                    println!(
                        "  \x1b[36m{:>width$}\x1b[0m │ {}",
                        gutter,
                        content,
                        width = max_gutter_width
                    );
                }
                if !missing.is_empty() {
                    let host_count = missing.len();
                    let gutter = if host_count == 1 {
                        missing[0].clone()
                    } else {
                        format!("[{}]", host_count)
                    };
                    println!(
                        "  \x1b[36m{:>width$}\x1b[0m │ \x1b[90m<missing>\x1b[0m",
                        gutter,
                        width = max_gutter_width
                    );
                }
            }
        }
    }

    Ok(())
}

/// Format status with ANSI color
fn format_status(status: &str) -> String {
    match status {
        "running" => format!("\x1b[33m{}\x1b[0m", status),  // yellow
        "success" => format!("\x1b[32m{}\x1b[0m", status),  // green
        "failed" => format!("\x1b[31m{}\x1b[0m", status),   // red
        _ => status.to_string(),
    }
}

fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    output_dir: &Path,
) -> Result<()> {
    let mut state = WatchState::new(output_dir.to_path_buf());
    let _ = state.refresh(); // Initial refresh, ignore errors
    if state.tail_mode {
        state.scroll_to_end();
    }

    loop {
        // Get spinner char (advances animation)
        let spinner = state.spinner_char();

        // Draw UI
        terminal.draw(|f| render_ui(f, &state, spinner))?;

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
            state.scroll_to_end();
        }
    }

    Ok(())
}

fn render_ui(f: &mut Frame, state: &WatchState, spinner: char) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Status bar
            Constraint::Min(1),    // Main content
            Constraint::Length(3), // Help bar
        ])
        .split(f.area());

    render_status_bar(f, chunks[0], state, spinner);
    render_consensus(f, chunks[1], state);
    render_help_bar(f, chunks[2]);
}

fn render_status_bar(f: &mut Frame, area: Rect, state: &WatchState, spinner: char) {
    let spinner_str = spinner.to_string();
    let status_items: Vec<Span> = state
        .hosts
        .iter()
        .flat_map(|host| {
            let status = state.statuses.get(host).map(|s| s.as_str()).unwrap_or("?");
            let (symbol, color): (&str, Color) = match status {
                "running" => (&spinner_str, Color::Yellow),
                "success" => ("✓", Color::Green),
                "failed" => ("✗", Color::Red),
                _ => ("?", Color::Gray),
            };
            vec![
                Span::raw(host.clone()),
                Span::raw(":"),
                Span::styled(symbol.to_string(), Style::default().fg(color)),
                Span::raw("  "),
            ]
        })
        .collect();

    let tail_indicator = if state.tail_mode { " [TAIL]" } else { "" };
    let keep_indicator = if state.keep_output { " [KEEP]" } else { "" };
    let title = format!(
        "Consensus View ({} hosts){}{}",
        state.hosts.len(),
        tail_indicator,
        keep_indicator
    );
    let paragraph = Paragraph::new(Line::from(status_items))
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}

fn render_consensus(f: &mut Frame, area: Rect, state: &WatchState) {
    let inner_height = area.height.saturating_sub(2) as usize; // Account for borders

    // Calculate scroll offset to keep selected line visible
    let mut scroll_offset: usize = 0;

    // Calculate display row of selected line/variant
    let mut selected_display_row = 0;
    for (i, line) in state.consensus.iter().enumerate() {
        if i == state.selected_line {
            // Add offset for selected variant within this line
            if let Some(var_idx) = state.selected_variant {
                selected_display_row += 1 + var_idx; // +1 for main line
            }
            break;
        }
        selected_display_row += match line {
            ConsensusLine::Identical(_) => 1,
            ConsensusLine::Differs {
                variants,
                missing,
                expanded,
                ..
            } => {
                if *expanded {
                    1 + variants.len() + if missing.is_empty() { 0 } else { 1 }
                } else {
                    1
                }
            }
        };
    }

    // Adjust scroll to keep selected in view
    if selected_display_row < scroll_offset {
        scroll_offset = selected_display_row;
    } else if selected_display_row >= scroll_offset + inner_height {
        scroll_offset = selected_display_row.saturating_sub(inner_height) + 1;
    }

    // Build display lines
    let mut lines: Vec<Line> = Vec::new();
    let mut current_row = 0;

    for (i, consensus_line) in state.consensus.iter().enumerate() {
        let is_selected = i == state.selected_line;

        match consensus_line {
            ConsensusLine::Identical(content) => {
                if current_row >= scroll_offset && current_row < scroll_offset + inner_height {
                    let style = if is_selected {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    };
                    lines.push(Line::from(Span::styled(content.clone(), style)));
                }
                current_row += 1;
            }
            ConsensusLine::Differs {
                consensus,
                consensus_count,
                total_hosts,
                variants,
                missing,
                expanded,
                expanded_hosts,
            } => {
                // Is the main line selected (not a variant)?
                let main_line_selected = is_selected && state.selected_variant.is_none();

                // Main line: show consensus content with diff indicator
                if current_row >= scroll_offset && current_row < scroll_offset + inner_height {
                    let diff_count = total_hosts - consensus_count;
                    let marker = if *expanded { "v" } else { ">" };

                    // Build the line with marker and content
                    let marker_style = if main_line_selected {
                        Style::default().fg(Color::Yellow).bg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::Yellow)
                    };

                    let content_style = if main_line_selected {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    };

                    let diff_indicator = format!("[{}{}] ", marker, diff_count);
                    lines.push(Line::from(vec![
                        Span::styled(diff_indicator, marker_style),
                        Span::styled(consensus.clone(), content_style),
                    ]));
                }
                current_row += 1;

                // Expanded variant details
                if *expanded {
                    let variant_count = variants.len();

                    // Calculate max gutter width for alignment
                    let max_gutter_width = variants
                        .iter()
                        .map(|(content, hosts)| {
                            let host_count = hosts.len();
                            if host_count == 1 {
                                hosts[0].len()
                            } else if expanded_hosts.contains(content) {
                                hosts.join(",").len()
                            } else {
                                format!("[{}]", host_count).len()
                            }
                        })
                        .chain(if missing.is_empty() {
                            None
                        } else {
                            let host_count = missing.len();
                            Some(if host_count == 1 {
                                missing[0].len()
                            } else if expanded_hosts.contains("<missing>") {
                                missing.join(",").len()
                            } else {
                                format!("[{}]", host_count).len()
                            })
                        })
                        .max()
                        .unwrap_or(4)
                        .max(4); // Minimum width of 4

                    for (idx, (content, hosts)) in variants.iter().enumerate() {
                        let variant_selected =
                            is_selected && state.selected_variant == Some(idx);

                        if current_row >= scroll_offset && current_row < scroll_offset + inner_height
                        {
                            let host_count = hosts.len();
                            let is_hosts_expanded = expanded_hosts.contains(content);

                            // Format the gutter based on host count and expansion state
                            let gutter = if host_count == 1 {
                                // Single host - show name
                                hosts[0].clone()
                            } else if is_hosts_expanded {
                                // Multiple hosts, expanded - show full list
                                hosts.join(",")
                            } else {
                                // Multiple hosts, collapsed - show [N]
                                format!("[{}]", host_count)
                            };

                            let gutter_style = if variant_selected {
                                Style::default().fg(Color::Cyan).bg(Color::DarkGray)
                            } else {
                                Style::default().fg(Color::Cyan)
                            };

                            let content_style = if variant_selected {
                                Style::default().fg(Color::Gray).bg(Color::DarkGray)
                            } else {
                                Style::default().fg(Color::Gray)
                            };

                            lines.push(Line::from(vec![
                                Span::styled(
                                    format!("  {:>width$} ", gutter, width = max_gutter_width),
                                    gutter_style,
                                ),
                                Span::styled(
                                    "│ ",
                                    if variant_selected {
                                        Style::default().fg(Color::DarkGray).bg(Color::DarkGray)
                                    } else {
                                        Style::default().fg(Color::DarkGray)
                                    },
                                ),
                                Span::styled(truncate(content, 60), content_style),
                            ]));
                        }
                        current_row += 1;
                    }

                    if !missing.is_empty() {
                        let missing_idx = variant_count;
                        let variant_selected =
                            is_selected && state.selected_variant == Some(missing_idx);

                        if current_row >= scroll_offset && current_row < scroll_offset + inner_height
                        {
                            let host_count = missing.len();
                            let is_hosts_expanded = expanded_hosts.contains("<missing>");

                            let gutter = if host_count == 1 {
                                missing[0].clone()
                            } else if is_hosts_expanded {
                                missing.join(",")
                            } else {
                                format!("[{}]", host_count)
                            };

                            let gutter_style = if variant_selected {
                                Style::default().fg(Color::Cyan).bg(Color::DarkGray)
                            } else {
                                Style::default().fg(Color::Cyan)
                            };

                            let content_style = if variant_selected {
                                Style::default().fg(Color::DarkGray).bg(Color::DarkGray)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            };

                            lines.push(Line::from(vec![
                                Span::styled(
                                    format!("  {:>width$} ", gutter, width = max_gutter_width),
                                    gutter_style,
                                ),
                                Span::styled(
                                    "│ ",
                                    if variant_selected {
                                        Style::default().bg(Color::DarkGray)
                                    } else {
                                        Style::default().fg(Color::DarkGray)
                                    },
                                ),
                                Span::styled("<missing>", content_style),
                            ]));
                        }
                        current_row += 1;
                    }
                }
            }
        }
    }

    if lines.is_empty() && state.hosts.is_empty() {
        lines.push(Line::from(Span::styled(
            "No host directories found...",
            Style::default().fg(Color::Yellow),
        )));
    } else if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no output yet)",
            Style::default().fg(Color::Gray),
        )));
    }

    let paragraph = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
    f.render_widget(paragraph, area);
}

fn render_help_bar(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Span::styled("↑↓", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":scroll  "),
        Span::styled("→←", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":expand/collapse  "),
        Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":next-diff  "),
        Span::styled("t", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":tail  "),
        Span::styled("e/c", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":all  "),
        Span::styled("K", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":keep  "),
        Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":quit"),
    ];

    let paragraph = Paragraph::new(Line::from(help_text))
        .block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(paragraph, area);
}

/// Truncate a string to a max length with ellipsis
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
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

/// Read output log for a host
fn read_output(output_dir: &Path, host: &str) -> String {
    let log_path = output_dir.join(host).join("out.log");
    let raw = fs::read_to_string(&log_path).unwrap_or_default();
    // Process carriage returns and clean up control characters
    clean_terminal_output(&raw)
}

/// Clean terminal output by processing carriage returns and stripping control chars
fn clean_terminal_output(raw: &str) -> String {
    raw.lines()
        .map(|line| {
            // Process carriage returns: text after \r overwrites from start of line
            let processed = if line.contains('\r') {
                let mut result = String::new();
                for segment in line.split('\r') {
                    if segment.is_empty() {
                        continue;
                    }
                    // Overwrite from the beginning, but keep any extra length
                    let segment_chars: Vec<char> = segment.chars().collect();
                    let result_chars: Vec<char> = result.chars().collect();

                    if segment_chars.len() >= result_chars.len() {
                        result = segment.to_string();
                    } else {
                        // Overwrite beginning, keep rest
                        let mut new_result: Vec<char> = result_chars;
                        for (i, c) in segment_chars.into_iter().enumerate() {
                            new_result[i] = c;
                        }
                        result = new_result.into_iter().collect();
                    }
                }
                result
            } else {
                line.to_string()
            };

            // Strip ANSI escape sequences and other control characters
            strip_ansi_and_control(&processed)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip ANSI escape sequences and control characters from a string
fn strip_ansi_and_control(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // ANSI escape sequence - skip until end
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Skip until we hit a letter (end of sequence)
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c.is_control() && c != '\t' {
            // Skip control characters except tab
        } else {
            result.push(c);
        }
    }

    result
}

/// Read status for a host (running, success, failed, or pending)
fn read_status(output_dir: &Path, host: &str) -> String {
    let status_path = output_dir.join(host).join("status");
    fs::read_to_string(&status_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "pending".to_string())
}

/// Compute consensus view from all host outputs using simple line-by-line comparison
fn compute_consensus(hosts: &[String], outputs: &HashMap<&str, String>) -> Vec<ConsensusLine> {
    if hosts.is_empty() {
        return Vec::new();
    }

    let total_hosts = hosts.len();

    // Parse all outputs into lines
    let host_lines: Vec<(&String, Vec<&str>)> = hosts
        .iter()
        .map(|h| (h, outputs[h.as_str()].lines().collect::<Vec<_>>()))
        .collect();

    // Find the maximum number of lines
    let max_lines = host_lines.iter().map(|(_, lines)| lines.len()).max().unwrap_or(0);

    // For each line position, collect what each host has
    (0..max_lines)
        .map(|line_idx| {
            let mut variants: IndexMap<String, Vec<String>> = IndexMap::new();
            let mut missing: Vec<String> = Vec::new();

            for (host, lines) in &host_lines {
                if let Some(&content) = lines.get(line_idx) {
                    variants
                        .entry(content.to_string())
                        .or_insert_with(Vec::new)
                        .push((*host).clone());
                } else {
                    missing.push((*host).clone());
                }
            }

            // If all hosts have the same content, it's identical
            if variants.len() == 1 && missing.is_empty() {
                ConsensusLine::Identical(variants.into_keys().next().unwrap())
            } else {
                make_differs(variants, missing, total_hosts)
            }
        })
        .collect()
}

/// Create a ConsensusLine::Differs with the most common variant as consensus
fn make_differs(
    variants: IndexMap<String, Vec<String>>,
    missing: Vec<String>,
    total_hosts: usize,
) -> ConsensusLine {
    // Find the most common variant (most hosts)
    let (consensus, consensus_hosts) = variants
        .iter()
        .max_by_key(|(_, hosts)| hosts.len())
        .map(|(content, hosts)| (content.clone(), hosts.len()))
        .unwrap_or_else(|| (String::new(), 0));

    // Sort variants by frequency (most common first)
    let mut sorted_variants: Vec<_> = variants.into_iter().collect();
    sorted_variants.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    let variants: IndexMap<String, Vec<String>> = sorted_variants.into_iter().collect();

    ConsensusLine::Differs {
        consensus,
        consensus_count: consensus_hosts,
        total_hosts,
        variants,
        missing,
        expanded: false,
        expanded_hosts: std::collections::HashSet::new(),
    }
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
}
