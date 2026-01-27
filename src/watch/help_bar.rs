use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    symbols::merge::MergeStrategy,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// HelpBar widget - displays keyboard shortcuts
pub(crate) struct HelpBar;

impl Widget for HelpBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
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

        let paragraph = Paragraph::new(Line::from(help_text)).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Help")
                .merge_borders(MergeStrategy::Exact),
        );
        paragraph.render(area, buf);
    }
}
