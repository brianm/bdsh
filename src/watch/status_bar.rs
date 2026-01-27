use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};
use std::collections::HashMap;

/// StatusBar widget - displays host status summary with colored indicators
pub(crate) struct StatusBar<'a> {
    pub(crate) hosts: &'a [String],
    pub(crate) statuses: &'a HashMap<String, String>,
    pub(crate) spinner: char,
    pub(crate) tail_mode: bool,
    pub(crate) keep_output: bool,
}

impl<'a> StatusBar<'a> {
    pub(crate) fn new(
        hosts: &'a [String],
        statuses: &'a HashMap<String, String>,
        spinner: char,
        tail_mode: bool,
        keep_output: bool,
    ) -> Self {
        Self {
            hosts,
            statuses,
            spinner,
            tail_mode,
            keep_output,
        }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let spinner_str = self.spinner.to_string();
        let status_items: Vec<Span> = self
            .hosts
            .iter()
            .flat_map(|host| {
                let status = self.statuses.get(host).map(|s| s.as_str()).unwrap_or("?");
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

        let tail_indicator = if self.tail_mode { " [TAIL]" } else { "" };
        let keep_indicator = if self.keep_output { " [KEEP]" } else { "" };
        let title = format!(
            "Consensus View ({} hosts){}{}",
            self.hosts.len(),
            tail_indicator,
            keep_indicator
        );
        let paragraph = Paragraph::new(Line::from(status_items))
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: true });
        paragraph.render(area, buf);
    }
}
