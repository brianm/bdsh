use indexmap::IndexMap;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};
use std::collections::{HashMap, HashSet};

/// A line in the consensus view
#[derive(Clone, Debug)]
pub(super) enum ConsensusLine {
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
        expanded_hosts: HashSet<String>,
    },
}

/// Selection - makes the two-level selection model explicit
#[derive(Clone, Debug)]
pub(crate) struct Selection {
    pub(crate) line_index: usize,
    /// None = main line selected, Some(idx) = variant at index selected
    pub(crate) variant_index: Option<usize>,
}

impl Selection {
    pub(crate) fn new() -> Self {
        Self {
            line_index: 0,
            variant_index: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn is_on_main_line(&self) -> bool {
        self.variant_index.is_none()
    }
}

/// ConsensusView component - all consensus rendering, navigation, expansion logic
pub(crate) struct ConsensusView {
    pub(crate) consensus: Vec<ConsensusLine>,
    pub(crate) selection: Selection,
    pub(crate) has_hosts: bool,
}

impl ConsensusView {
    pub(crate) fn new() -> Self {
        Self {
            consensus: Vec::new(),
            selection: Selection::new(),
            has_hosts: false,
        }
    }

    /// Update consensus data
    pub(crate) fn update_consensus(&mut self, new_consensus: Vec<ConsensusLine>, has_hosts: bool) {
        self.consensus = new_consensus;
        self.has_hosts = has_hosts;
        self.clamp_selection();
    }

    /// Ensure selection is within valid bounds
    fn clamp_selection(&mut self) {
        let max_line = self.consensus.len().saturating_sub(1);
        if self.selection.line_index > max_line {
            self.selection.line_index = max_line;
        }
    }

    /// Scroll to the end of the consensus view
    pub(crate) fn scroll_to_end(&mut self) {
        if !self.consensus.is_empty() {
            self.selection.line_index = self.consensus.len() - 1;
            // If last line is expanded Differs, select last variant
            if let Some(ConsensusLine::Differs {
                expanded: true,
                variants,
                missing,
                ..
            }) = self.consensus.get(self.selection.line_index)
            {
                let variant_count = variants.len() + if missing.is_empty() { 0 } else { 1 };
                if variant_count > 0 {
                    self.selection.variant_index = Some(variant_count - 1);
                }
            } else {
                self.selection.variant_index = None;
            }
        }
    }

    /// Scroll up one position
    pub(crate) fn scroll_up(&mut self) {
        // If we're in a variant, try to move up within variants
        if let Some(var_idx) = self.selection.variant_index {
            if var_idx > 0 {
                self.selection.variant_index = Some(var_idx - 1);
                return;
            } else {
                // At first variant, exit to main line
                self.selection.variant_index = None;
                return;
            }
        }

        // Move to previous consensus line
        if self.selection.line_index > 0 {
            self.selection.line_index -= 1;
            // If previous line is expanded Differs, select its last variant
            if let Some(ConsensusLine::Differs {
                expanded: true,
                variants,
                missing,
                ..
            }) = self.consensus.get(self.selection.line_index)
            {
                let variant_count = variants.len() + if missing.is_empty() { 0 } else { 1 };
                if variant_count > 0 {
                    self.selection.variant_index = Some(variant_count - 1);
                }
            }
        }
    }

    /// Scroll down one position
    pub(crate) fn scroll_down(&mut self) {
        // Check if current line is an expanded Differs
        if let Some(ConsensusLine::Differs {
            expanded: true,
            variants,
            missing,
            ..
        }) = self.consensus.get(self.selection.line_index)
        {
            let variant_count = variants.len() + if missing.is_empty() { 0 } else { 1 };

            if let Some(var_idx) = self.selection.variant_index {
                if var_idx + 1 < variant_count {
                    // Move to next variant
                    self.selection.variant_index = Some(var_idx + 1);
                    return;
                } else {
                    // At last variant, move to next consensus line
                    self.selection.variant_index = None;
                    if self.selection.line_index < self.consensus.len().saturating_sub(1) {
                        self.selection.line_index += 1;
                    }
                    return;
                }
            } else {
                // On main line of expanded Differs, enter variants
                if variant_count > 0 {
                    self.selection.variant_index = Some(0);
                    return;
                }
            }
        }

        // Move to next consensus line
        self.selection.variant_index = None;
        if self.selection.line_index < self.consensus.len().saturating_sub(1) {
            self.selection.line_index += 1;
        }
    }

    /// Toggle expansion of the selected diff line
    pub(crate) fn toggle_expand(&mut self) {
        if let Some(ConsensusLine::Differs { expanded, .. }) =
            self.consensus.get_mut(self.selection.line_index)
        {
            *expanded = !*expanded;
        }
    }

    /// Expand the selected line/variant (right-arrow behavior)
    pub(crate) fn expand_selected(&mut self) {
        if let Some(ConsensusLine::Differs {
            expanded,
            variants,
            missing,
            expanded_hosts,
            ..
        }) = self.consensus.get_mut(self.selection.line_index)
        {
            if !*expanded {
                // Expand to show variants
                *expanded = true;
            } else if let Some(var_idx) = self.selection.variant_index {
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
    pub(crate) fn collapse_selected(&mut self) {
        if let Some(ConsensusLine::Differs {
            expanded,
            variants,
            expanded_hosts,
            ..
        }) = self.consensus.get_mut(self.selection.line_index)
        {
            if let Some(var_idx) = self.selection.variant_index {
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
                    self.selection.variant_index = Some(var_idx - 1);
                } else {
                    self.selection.variant_index = None;
                }
            } else if *expanded {
                // Collapse the variants
                *expanded = false;
                expanded_hosts.clear();
            }
        }
    }

    /// Expand all diff lines
    pub(crate) fn expand_all(&mut self) {
        for line in &mut self.consensus {
            if let ConsensusLine::Differs { expanded, .. } = line {
                *expanded = true;
            }
        }
    }

    /// Collapse all diff lines
    pub(crate) fn collapse_all(&mut self) {
        for line in &mut self.consensus {
            if let ConsensusLine::Differs { expanded, .. } = line {
                *expanded = false;
            }
        }
    }

    /// Jump to the next difference
    pub(crate) fn jump_to_next_diff(&mut self) {
        let start = self.selection.line_index + 1;
        for i in start..self.consensus.len() {
            if matches!(self.consensus[i], ConsensusLine::Differs { .. }) {
                self.selection.line_index = i;
                return;
            }
        }
        // Wrap around
        for i in 0..start.min(self.consensus.len()) {
            if matches!(self.consensus[i], ConsensusLine::Differs { .. }) {
                self.selection.line_index = i;
                return;
            }
        }
    }

    /// Calculate scroll offset to keep selected line visible
    pub(crate) fn calculate_scroll_offset(&self, viewport_height: usize) -> usize {
        let selected_display_row = self.selected_display_row();

        let mut scroll_offset = 0;

        // Adjust scroll to keep selected in view
        if selected_display_row < scroll_offset {
            scroll_offset = selected_display_row;
        } else if selected_display_row >= scroll_offset + viewport_height {
            scroll_offset = selected_display_row.saturating_sub(viewport_height) + 1;
        }

        scroll_offset
    }

    /// Calculate display row of selected line/variant
    fn selected_display_row(&self) -> usize {
        let mut display_row = 0;
        for (i, line) in self.consensus.iter().enumerate() {
            if i == self.selection.line_index {
                // Add offset for selected variant within this line
                if let Some(var_idx) = self.selection.variant_index {
                    display_row += 1 + var_idx; // +1 for main line
                }
                break;
            }
            display_row += match line {
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
        display_row
    }

    /// Build display lines for rendering
    pub(crate) fn build_display_lines(
        &self,
        scroll_offset: usize,
        viewport_height: usize,
    ) -> Vec<Line<'_>> {
        let mut lines: Vec<Line> = Vec::new();
        let mut current_row = 0;

        for (i, consensus_line) in self.consensus.iter().enumerate() {
            let is_selected = i == self.selection.line_index;

            match consensus_line {
                ConsensusLine::Identical(content) => {
                    if current_row >= scroll_offset && current_row < scroll_offset + viewport_height {
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
                    let main_line_selected = is_selected && self.selection.variant_index.is_none();

                    // Main line: show consensus content with diff indicator
                    if current_row >= scroll_offset && current_row < scroll_offset + viewport_height {
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
                                is_selected && self.selection.variant_index == Some(idx);

                            if current_row >= scroll_offset && current_row < scroll_offset + viewport_height
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
                                is_selected && self.selection.variant_index == Some(missing_idx);

                            if current_row >= scroll_offset && current_row < scroll_offset + viewport_height
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

        if lines.is_empty() && !self.has_hosts {
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

        lines
    }
}

/// Truncate a string to a maximum length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Compute consensus view from all host outputs using simple line-by-line comparison
pub(crate) fn compute_consensus(
    hosts: &[String],
    outputs: &HashMap<&str, String>,
) -> Vec<ConsensusLine> {
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
    let max_lines = host_lines
        .iter()
        .map(|(_, lines)| lines.len())
        .max()
        .unwrap_or(0);

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
        expanded_hosts: HashSet::new(),
    }
}

/// Clean terminal output by processing carriage returns and stripping ANSI codes
pub(crate) fn clean_terminal_output(raw: &str) -> String {
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

/// ConsensusViewWidget - zero-sized widget for rendering ConsensusView state
pub(super) struct ConsensusViewWidget;

impl StatefulWidget for ConsensusViewWidget {
    type State = ConsensusView;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ConsensusView) {
        let inner_height = area.height.saturating_sub(2) as usize; // Account for borders

        // Calculate scroll offset to keep selected line visible
        let scroll_offset = state.calculate_scroll_offset(inner_height);

        // Build display lines
        let lines = state.build_display_lines(scroll_offset, inner_height);

        let paragraph = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
        paragraph.render(area, buf);
    }
}
