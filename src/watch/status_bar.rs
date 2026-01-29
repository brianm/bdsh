use crate::colors::ColorScheme;
use crate::Status;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    symbols::merge::MergeStrategy,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};
use std::collections::HashMap;

/// StatusBar widget - displays host status summary with colored indicators
pub(crate) struct StatusBar<'a> {
    pub(crate) hosts: &'a [String],
    pub(crate) statuses: &'a HashMap<String, Status>,
    pub(crate) waiting_for_input: &'a HashMap<String, bool>,
    pub(crate) spinner: char,
    pub(crate) spinner_frame: usize,
    pub(crate) tail_mode: bool,
    pub(crate) keep_output: bool,
    pub(crate) colors: &'a ColorScheme,
}

impl<'a> StatusBar<'a> {
    pub(crate) fn new(
        hosts: &'a [String],
        statuses: &'a HashMap<String, Status>,
        waiting_for_input: &'a HashMap<String, bool>,
        spinner: char,
        spinner_frame: usize,
        tail_mode: bool,
        keep_output: bool,
        colors: &'a ColorScheme,
    ) -> Self {
        Self {
            hosts,
            statuses,
            waiting_for_input,
            spinner,
            spinner_frame,
            tail_mode,
            keep_output,
            colors,
        }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let spinner_str = self.spinner.to_string();
        // Blink the input indicator at a slower rate than the spinner (~400ms on/off)
        let show_input_indicator = (self.spinner_frame / 5) % 2 == 0;

        let status_items: Vec<Span> = self
            .hosts
            .iter()
            .enumerate()
            .flat_map(|(idx, host)| {
                let status = self.statuses.get(host).copied().unwrap_or(Status::Pending);
                let is_waiting = self.waiting_for_input.get(host).copied().unwrap_or(false);

                let mut spans = vec![
                    Span::raw(host.clone()),
                    Span::raw(":"),
                ];

                // If waiting for input, show pulsing keyboard indicator instead of spinner
                // Window number is idx + 1 (window 0 is watch)
                if is_waiting {
                    let window_num = idx + 1;
                    let indicator = format!("⌨[{}]", window_num);
                    // Pulse between bright and dim magenta
                    let color = if show_input_indicator {
                        self.colors.input_waiting()
                    } else {
                        self.colors.input_waiting_dim()
                    };
                    spans.push(Span::styled(indicator, Style::default().fg(color)));
                } else {
                    let (symbol, color) = match status {
                        Status::Running => (spinner_str.as_str(), self.colors.running()),
                        Status::Success => ("✓", self.colors.success()),
                        Status::Failed => ("✗", self.colors.failed()),
                        Status::Pending => ("?", self.colors.pending()),
                    };
                    spans.push(Span::styled(symbol.to_string(), Style::default().fg(color)));
                }

                spans.push(Span::raw("  "));
                spans
            })
            .collect();

        let tail_indicator = if self.tail_mode { " [TAIL]" } else { "" };
        let keep_indicator = if self.keep_output { " [KEEP]" } else { "" };
        let title = format!(
            "Consensus View ({} hosts){}{}",
            self.hosts.len(),
            tail_indicator,
            keep_indicator
        );
        let paragraph = Paragraph::new(Line::from(status_items))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .merge_borders(MergeStrategy::Exact),
            )
            .wrap(Wrap { trim: true });
        paragraph.render(area, buf);
    }
}
