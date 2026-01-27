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
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::fs;
use std::io::{self, stdout, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// A line in the consensus view
#[derive(Clone, Debug)]
enum ConsensusLine {
    /// All hosts have identical content
    Identical(String),
    /// Hosts have different content
    Differs {
        /// Map from content -> list of hosts with that content (ordered)
        variants: IndexMap<String, Vec<String>>,
        /// Hosts missing this line entirely
        missing: Vec<String>,
        /// Currently expanded?
        expanded: bool,
    },
}

/// State for the watch mode TUI
struct WatchState {
    output_dir: PathBuf,
    hosts: Vec<String>,
    statuses: HashMap<String, String>,
    consensus: Vec<ConsensusLine>,
    selected_line: usize,
    /// Cache of last-read outputs to detect changes
    last_outputs: HashMap<String, String>,
}

impl WatchState {
    fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            hosts: Vec::new(),
            statuses: HashMap::new(),
            consensus: Vec::new(),
            selected_line: 0,
            last_outputs: HashMap::new(),
        }
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
        if self.selected_line > 0 {
            self.selected_line -= 1;
        }
    }

    fn scroll_down(&mut self) {
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
            ConsensusLine::Differs { variants, missing, .. } => {
                let variant_count = variants.len() + if missing.is_empty() { 0 } else { 1 };
                println!("\x1b[36m[{} variants]\x1b[0m", variant_count);
                for (content, hosts) in variants.iter() {
                    println!("  | {} ({})", content, hosts.join(", "));
                }
                if !missing.is_empty() {
                    println!("  | <missing> ({})", missing.join(", "));
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

    loop {
        // Draw UI
        terminal.draw(|f| render_ui(f, &state))?;

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

                        // Actions
                        (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => state.toggle_expand(),
                        (KeyCode::Tab, _) => state.jump_to_next_diff(),
                        (KeyCode::Char('e'), KeyModifiers::NONE) => state.expand_all(),
                        (KeyCode::Char('c'), KeyModifiers::NONE) => state.collapse_all(),

                        _ => {}
                    }
                }
            }
        }

        // Always refresh - reading small files is fast, and this avoids
        // any delays from file watcher event propagation
        let _ = state.refresh();
    }

    Ok(())
}

fn render_ui(f: &mut Frame, state: &WatchState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Status bar
            Constraint::Min(1),    // Main content
            Constraint::Length(3), // Help bar
        ])
        .split(f.area());

    render_status_bar(f, chunks[0], state);
    render_consensus(f, chunks[1], state);
    render_help_bar(f, chunks[2]);
}

fn render_status_bar(f: &mut Frame, area: Rect, state: &WatchState) {
    let status_items: Vec<Span> = state
        .hosts
        .iter()
        .flat_map(|host| {
            let status = state.statuses.get(host).map(|s| s.as_str()).unwrap_or("?");
            let (symbol, color) = match status {
                "running" => ("*", Color::Yellow),
                "success" => ("ok", Color::Green),
                "failed" => ("!!", Color::Red),
                _ => ("?", Color::Gray),
            };
            vec![
                Span::raw(host.clone()),
                Span::raw(":"),
                Span::styled(symbol, Style::default().fg(color)),
                Span::raw("  "),
            ]
        })
        .collect();

    let title = format!("Consensus View ({} hosts)", state.hosts.len());
    let paragraph = Paragraph::new(Line::from(status_items))
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}

fn render_consensus(f: &mut Frame, area: Rect, state: &WatchState) {
    let inner_height = area.height.saturating_sub(2) as usize; // Account for borders

    // Calculate scroll offset to keep selected line visible
    let mut scroll_offset: usize = 0;

    // Calculate display row of selected line
    let mut selected_display_row = 0;
    for (i, line) in state.consensus.iter().enumerate() {
        if i == state.selected_line {
            break;
        }
        selected_display_row += match line {
            ConsensusLine::Identical(_) => 1,
            ConsensusLine::Differs {
                variants,
                missing,
                expanded,
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
                variants,
                missing,
                expanded,
            } => {
                // Header line
                if current_row >= scroll_offset && current_row < scroll_offset + inner_height {
                    let marker = if *expanded { "v" } else { ">" };
                    let variant_count = variants.len() + if missing.is_empty() { 0 } else { 1 };
                    let preview = variants
                        .keys()
                        .next()
                        .map(|s| truncate(s, 30))
                        .unwrap_or_default();

                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    };

                    let line_content =
                        format!("{} [{} variants] {}...", marker, variant_count, preview);
                    lines.push(Line::from(Span::styled(line_content, style)));
                }
                current_row += 1;

                // Expanded variant details
                if *expanded {
                    for (content, hosts) in variants.iter() {
                        if current_row >= scroll_offset && current_row < scroll_offset + inner_height
                        {
                            let host_list = hosts.join(", ");
                            let detail = format!("  | {} ({})", truncate(content, 50), host_list);
                            lines.push(Line::from(Span::styled(
                                detail,
                                Style::default().fg(Color::Gray),
                            )));
                        }
                        current_row += 1;
                    }

                    if !missing.is_empty() {
                        if current_row >= scroll_offset && current_row < scroll_offset + inner_height
                        {
                            let host_list = missing.join(", ");
                            let detail = format!("  | <missing> ({})", host_list);
                            lines.push(Line::from(Span::styled(
                                detail,
                                Style::default().fg(Color::DarkGray),
                            )));
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
        Span::styled("j/k", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":scroll  "),
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":expand  "),
        Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":next-diff  "),
        Span::styled("e", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":expand-all  "),
        Span::styled("c", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(":collapse-all  "),
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
    fs::read_to_string(&log_path).unwrap_or_default()
}

/// Read status for a host (running, success, failed, or pending)
fn read_status(output_dir: &Path, host: &str) -> String {
    let status_path = output_dir.join(host).join("status");
    fs::read_to_string(&status_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "pending".to_string())
}

/// Compute consensus view from all host outputs
fn compute_consensus(hosts: &[String], outputs: &HashMap<&str, String>) -> Vec<ConsensusLine> {
    if hosts.is_empty() {
        return Vec::new();
    }

    if hosts.len() == 1 {
        // Single host - all lines are identical
        let host = &hosts[0];
        let output = &outputs[host.as_str()];
        return output
            .lines()
            .map(|line| ConsensusLine::Identical(line.to_string()))
            .collect();
    }

    // Use first host as baseline
    let baseline_host = &hosts[0];
    let baseline = &outputs[baseline_host.as_str()];
    let baseline_lines: Vec<&str> = baseline.lines().collect();

    // Track unified view: baseline line index -> UnifiedEntry
    // We'll build a more sophisticated structure to handle insertions
    #[derive(Clone, Debug)]
    enum UnifiedEntry {
        Baseline {
            variants: IndexMap<String, Vec<String>>,
            missing: Vec<String>,
        },
        Inserted {
            variants: IndexMap<String, Vec<String>>,
        },
    }

    // Initialize with baseline lines
    let mut unified: Vec<UnifiedEntry> = baseline_lines
        .iter()
        .map(|&line| {
            let mut variants = IndexMap::new();
            variants.insert(line.to_string(), vec![baseline_host.clone()]);
            UnifiedEntry::Baseline {
                variants,
                missing: Vec::new(),
            }
        })
        .collect();

    // Process each other host
    for host in hosts.iter().skip(1) {
        let host_output = &outputs[host.as_str()];
        let diff = TextDiff::from_lines(baseline, host_output);

        let mut baseline_idx = 0;
        let mut insertions: Vec<(usize, String, String)> = Vec::new(); // (position, content, host)

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Equal => {
                    // Host has same line as baseline at this position
                    if baseline_idx < unified.len() {
                        if let UnifiedEntry::Baseline { variants, .. } = &mut unified[baseline_idx] {
                            let content = change.value().trim_end().to_string();
                            variants
                                .entry(content)
                                .or_insert_with(Vec::new)
                                .push(host.clone());
                        }
                    }
                    baseline_idx += 1;
                }
                ChangeTag::Delete => {
                    // Baseline line missing in this host
                    if baseline_idx < unified.len() {
                        if let UnifiedEntry::Baseline { missing, .. } = &mut unified[baseline_idx] {
                            missing.push(host.clone());
                        }
                    }
                    baseline_idx += 1;
                }
                ChangeTag::Insert => {
                    // Host has extra line
                    let content = change.value().trim_end().to_string();
                    insertions.push((baseline_idx, content, host.clone()));
                }
            }
        }

        // Process insertions (in reverse to maintain indices)
        for (pos, content, host) in insertions.into_iter().rev() {
            // Clamp position to valid range - positions are relative to baseline
            // but unified may have grown from previous hosts' insertions
            let pos = pos.min(unified.len());

            // Check if there's already an Inserted at this position we can merge into
            let mut merged = false;
            if pos < unified.len() {
                if let UnifiedEntry::Inserted { variants } = &mut unified[pos] {
                    variants
                        .entry(content.clone())
                        .or_insert_with(Vec::new)
                        .push(host.clone());
                    merged = true;
                }
            }
            if !merged {
                let mut variants = IndexMap::new();
                variants.insert(content, vec![host]);
                unified.insert(pos, UnifiedEntry::Inserted { variants });
            }
        }
    }

    // Convert to display model
    unified
        .into_iter()
        .map(|entry| match entry {
            UnifiedEntry::Baseline { variants, missing } => {
                if variants.len() == 1 && missing.is_empty() {
                    ConsensusLine::Identical(variants.into_keys().next().unwrap())
                } else {
                    ConsensusLine::Differs {
                        variants: variants.into_iter().collect(),
                        missing,
                        expanded: false,
                    }
                }
            }
            UnifiedEntry::Inserted { variants } => {
                if variants.len() == 1 {
                    // Check if all hosts have this insertion (meaning it's actually identical)
                    let (content, hosts_with) = variants.into_iter().next().unwrap();
                    if hosts_with.len() == hosts.len() - 1 {
                        // All non-baseline hosts have this - it's actually a deletion from baseline
                        ConsensusLine::Differs {
                            variants: IndexMap::from([(content, hosts_with)]),
                            missing: vec![hosts[0].clone()], // baseline is "missing" this
                            expanded: false,
                        }
                    } else {
                        ConsensusLine::Differs {
                            variants: IndexMap::from([(content, hosts_with)]),
                            missing: Vec::new(),
                            expanded: false,
                        }
                    }
                } else {
                    ConsensusLine::Differs {
                        variants: variants.into_iter().collect(),
                        missing: Vec::new(),
                        expanded: false,
                    }
                }
            }
        })
        .collect()
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

        // The diff algorithm treats replacement as delete + insert, so we get 4 lines:
        // line1 (identical), line2 (host1+host3, missing host2), DIFFERENT (host2 only), line3 (identical)
        assert_eq!(consensus.len(), 4);
        assert!(matches!(&consensus[0], ConsensusLine::Identical(s) if s == "line1"));
        // line2 is present in host1+host3, missing in host2
        assert!(matches!(&consensus[1], ConsensusLine::Differs { missing, .. } if missing.contains(&"host2".to_string())));
        // DIFFERENT is inserted by host2
        assert!(matches!(&consensus[2], ConsensusLine::Differs { variants, .. } if variants.contains_key("DIFFERENT")));
        assert!(matches!(&consensus[3], ConsensusLine::Identical(s) if s == "line3"));
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
