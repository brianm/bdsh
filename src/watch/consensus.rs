use indexmap::IndexMap;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    symbols::merge::MergeStrategy,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};
use std::collections::{HashMap, HashSet};

/// Format a host gutter string based on host count and expansion state.
///
/// - Single host: returns the hostname
/// - Multiple hosts, expanded: returns comma-separated list
/// - Multiple hosts, collapsed: returns `[N]` where N is count
pub(super) fn format_gutter(hosts: &[String], expanded: bool) -> String {
    match hosts.len() {
        1 => hosts[0].clone(),
        n if expanded => hosts.join(","),
        n => format!("[{}]", n),
    }
}

/// Calculate the display width of a gutter entry.
pub(super) fn gutter_width(hosts: &[String], expanded: bool) -> usize {
    format_gutter(hosts, expanded).len()
}

/// Calculate the maximum gutter width needed for alignment across all variants.
///
/// Takes variant hosts, missing hosts, and an optional set of expanded host keys.
/// Returns the max width with a minimum of 4 characters.
pub(super) fn max_gutter_width(
    variants: &IndexMap<String, Vec<String>>,
    missing: &[String],
    expanded_hosts: Option<&HashSet<String>>,
) -> usize {
    let empty = HashSet::new();
    let expanded = expanded_hosts.unwrap_or(&empty);

    variants
        .iter()
        .map(|(content, hosts)| gutter_width(hosts, expanded.contains(content)))
        .chain(if missing.is_empty() {
            None
        } else {
            Some(gutter_width(missing, expanded.contains("<missing>")))
        })
        .max()
        .unwrap_or(4)
        .max(4)
}

/// A line in the consensus view
#[derive(Clone, Debug)]
pub(super) enum ConsensusLine {
    /// All hosts have identical content
    Identical(String),
    /// Hosts have different content - show most common, allow expansion
    Differs {
        /// The most common content (shown by default)
        consensus: String,
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

/// Two-level selection model for hierarchical consensus navigation.
///
/// The consensus view displays a list of lines, where some lines (Differs) can be
/// expanded to show variants. This creates a two-level hierarchy:
///
/// ```text
/// Line 0: "identical line"              <- line_index=0, variant_index=None
/// Line 1: "[2] consensus output"        <- line_index=1, variant_index=None (collapsed)
/// Line 2: "[v2] consensus output"       <- line_index=2, variant_index=None (expanded, main)
///           host1 │ "variant A"         <- line_index=2, variant_index=Some(0)
///           host2 │ "variant B"         <- line_index=2, variant_index=Some(1)
///            [2]  │ <missing>           <- line_index=2, variant_index=Some(2)
/// Line 3: "another line"                <- line_index=3, variant_index=None
/// ```
///
/// Navigation behavior:
/// - ↓ on collapsed Differs: move to next line
/// - ↓ on expanded Differs main line: enter variants (variant_index = Some(0))
/// - ↓ on last variant: exit to next line
/// - ↑ reverses this behavior
/// - →/← expand/collapse the selected item
#[derive(Clone, Debug, Default)]
pub(crate) struct Selection {
    /// Index into the consensus lines vector
    pub(crate) line_index: usize,
    /// None = main line selected, Some(idx) = variant at index within expanded Differs
    pub(crate) variant_index: Option<usize>,
}

impl Selection {
    #[allow(dead_code)]
    pub(crate) fn is_on_main_line(&self) -> bool {
        self.variant_index.is_none()
    }
}

/// ConsensusView - manages consensus data, selection state, and rendering.
///
/// Handles the hierarchical display of command output differences across hosts,
/// including expansion/collapse state and keyboard navigation.
#[derive(Default)]
pub(crate) struct ConsensusView {
    pub(crate) consensus: Vec<ConsensusLine>,
    pub(crate) selection: Selection,
    pub(crate) has_hosts: bool,
}

impl ConsensusView {
    pub(crate) fn new() -> Self {
        Self::default()
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
                    variants,
                    missing,
                    expanded,
                    expanded_hosts,
                    ..
                } => {
                    // Is the main line selected (not a variant)?
                    let main_line_selected = is_selected && self.selection.variant_index.is_none();

                    // Main line: show consensus content with variant count indicator
                    if current_row >= scroll_offset && current_row < scroll_offset + viewport_height {
                        let variant_count = variants.len();
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

                        let diff_indicator = format!("{}[{}] ", marker, variant_count);
                        lines.push(Line::from(vec![
                            Span::styled(diff_indicator, marker_style),
                            Span::styled(consensus.clone(), content_style),
                        ]));
                    }
                    current_row += 1;

                    // Expanded variant details
                    if *expanded {
                        let variant_count = variants.len();
                        let max_width = max_gutter_width(variants, missing, Some(expanded_hosts));

                        for (idx, (content, hosts)) in variants.iter().enumerate() {
                            let variant_selected =
                                is_selected && self.selection.variant_index == Some(idx);

                            if current_row >= scroll_offset && current_row < scroll_offset + viewport_height
                            {
                                let is_hosts_expanded = expanded_hosts.contains(content);
                                let gutter = format_gutter(hosts, is_hosts_expanded);

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
                                        format!("  {:>width$} ", gutter, width = max_width),
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
                                let is_hosts_expanded = expanded_hosts.contains("<missing>");
                                let gutter = format_gutter(missing, is_hosts_expanded);

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
                                        format!("  {:>width$} ", gutter, width = max_width),
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

/// Truncate a string to a maximum number of characters (not bytes).
/// Handles multi-byte UTF-8 characters safely.
fn truncate(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
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
                make_differs(variants, missing)
            }
        })
        .collect()
}

/// Create a ConsensusLine::Differs with the most common variant as consensus
fn make_differs(
    variants: IndexMap<String, Vec<String>>,
    missing: Vec<String>,
) -> ConsensusLine {
    // Find the most common variant (most hosts)
    let consensus = variants
        .iter()
        .max_by_key(|(_, hosts)| hosts.len())
        .map(|(content, _)| content.clone())
        .unwrap_or_default();

    // Sort variants by frequency (most common first)
    let mut sorted_variants: Vec<_> = variants.into_iter().collect();
    sorted_variants.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    let variants: IndexMap<String, Vec<String>> = sorted_variants.into_iter().collect();

    ConsensusLine::Differs {
        consensus,
        variants,
        missing,
        expanded: false,
        expanded_hosts: HashSet::new(),
    }
}

/// Clean terminal output by processing carriage returns and stripping ANSI codes.
///
/// Carriage return (`\r`) handling simulates terminal behavior where `\r` moves
/// the cursor to the start of the line and subsequent text overwrites from there:
///
/// - `"hello\rhi"` → `"hillo"` (overwrites first 2 chars, keeps rest)
/// - `"hello\rworld"` → `"world"` (full overwrite, new text is longer)
/// - `"a\rb\rc"` → `"c"` (multiple CRs, each overwrites from start)
/// - `"loading...\rdone      "` → `"done      "` (progress indicator pattern)
///
/// This is commonly seen in progress bars, spinners, and status updates.
pub(crate) fn clean_terminal_output(raw: &str) -> String {
    raw.lines()
        .map(|line| {
            // Strip ANSI escape sequences FIRST (before CR processing mangles them)
            let stripped = strip_ansi_and_control(line);

            // Process carriage returns: text after \r overwrites from start of line
            if stripped.contains('\r') {
                let mut result = String::new();
                for segment in stripped.split('\r') {
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
                stripped
            }
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
            match chars.peek() {
                // CSI sequence: \x1b[ ... <letter>
                Some(&'[') => {
                    chars.next(); // consume '['
                    // Skip until we hit a letter (end of sequence)
                    while let Some(&next) = chars.peek() {
                        chars.next();
                        if next.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                // OSC sequence: \x1b] ... \x07 or \x1b\\ or \x9c
                Some(&']') => {
                    chars.next(); // consume ']'
                    // Skip until terminator (BEL, ST, or 8-bit ST)
                    while let Some(&next) = chars.peek() {
                        if next == '\x07' || next == '\u{9c}' {
                            chars.next();
                            break;
                        } else if next == '\x1b' {
                            chars.next();
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                        chars.next();
                    }
                }
                // DCS sequence: \x1bP ... \x1b\\ or \x9c
                Some(&'P') => {
                    chars.next(); // consume 'P'
                    // Skip until ST terminator
                    while let Some(&next) = chars.peek() {
                        if next == '\u{9c}' {
                            chars.next();
                            break;
                        } else if next == '\x1b' {
                            chars.next();
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                        chars.next();
                    }
                }
                // Other escape sequences - skip next char
                _ => {
                    chars.next();
                }
            }
        } else if c == '\u{9d}' {
            // 8-bit OSC introducer - skip until terminator
            while let Some(&next) = chars.peek() {
                if next == '\x07' || next == '\u{9c}' {
                    chars.next();
                    break;
                } else if next == '\x1b' {
                    chars.next();
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                    }
                    break;
                }
                chars.next();
            }
        } else if c == ']' && chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            // Bare ] followed by digits - likely partial OSC sequence, skip to terminator or space
            while let Some(&next) = chars.peek() {
                if next == '\x07' || next == '\u{9c}' || next == ' ' || next == '\n' {
                    if next == '\x07' || next == '\u{9c}' {
                        chars.next();
                    }
                    break;
                } else if next == '\x1b' {
                    chars.next();
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                    }
                    break;
                }
                chars.next();
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

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .merge_borders(MergeStrategy::Exact),
        );
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_ascii() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 10), "hello w...");
        assert_eq!(truncate("hi", 2), "hi");
    }

    #[test]
    fn test_truncate_unicode() {
        // Emoji are multi-byte but should count as single characters
        assert_eq!(truncate("🎉🎊🎁", 10), "🎉🎊🎁");
        assert_eq!(truncate("🎉🎊🎁🎄🎅🎆🎇", 5), "🎉🎊...");

        // Mixed ASCII and unicode
        assert_eq!(truncate("hello 🌍", 10), "hello 🌍");
        assert_eq!(truncate("hello 🌍 world", 10), "hello 🌍...");
    }

    #[test]
    fn test_truncate_edge_cases() {
        assert_eq!(truncate("", 5), "");
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("abcd", 3), "...");
    }
}
