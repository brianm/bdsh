use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    symbols::merge::MergeStrategy,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Which set of help/prompt content to render, depending on the active view mode.
pub(crate) enum HelpContext<'a> {
    Consensus,
    /// User is typing a host index; carries the current buffer.
    NumberEntry(&'a str),
    /// Single-host log view; `tail` follows new output.
    Log { tail: bool },
}

/// HelpBar widget - displays keyboard shortcuts (or the host-index prompt)
pub(crate) struct HelpBar<'a> {
    ctx: HelpContext<'a>,
}

impl<'a> HelpBar<'a> {
    pub(crate) fn new(ctx: HelpContext<'a>) -> Self {
        Self { ctx }
    }
}

fn key(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().add_modifier(Modifier::BOLD))
}

impl Widget for HelpBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (title, spans): (&str, Vec<Span>) = match self.ctx {
            HelpContext::Consensus => (
                "Help",
                vec![
                    key("↑↓"),
                    Span::raw(":scroll  "),
                    key("→←"),
                    Span::raw(":expand/collapse  "),
                    key("Tab"),
                    Span::raw(":next-diff  "),
                    key("t"),
                    Span::raw(":tail  "),
                    key("e/c"),
                    Span::raw(":all  "),
                    key("v"),
                    Span::raw(":log  "),
                    key("K"),
                    Span::raw(":keep  "),
                    key("q"),
                    Span::raw(":quit"),
                ],
            ),
            HelpContext::NumberEntry(buffer) => (
                "Select host",
                vec![
                    Span::raw("Open log for host #: "),
                    Span::styled(
                        format!("{}█", buffer),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("   "),
                    key("Enter"),
                    Span::raw(":open  "),
                    key("Esc"),
                    Span::raw(":cancel"),
                ],
            ),
            HelpContext::Log { tail } => {
                let title = if tail { "Help [TAIL]" } else { "Help" };
                (
                    title,
                    vec![
                        key("↑↓/jk"),
                        Span::raw(":scroll  "),
                        key("g/G"),
                        Span::raw(":top/bottom  "),
                        key("t"),
                        Span::raw(":tail  "),
                        key("q/Esc"),
                        Span::raw(":back"),
                    ],
                )
            }
        };

        let paragraph = Paragraph::new(Line::from(spans)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .merge_borders(MergeStrategy::Exact),
        );
        paragraph.render(area, buf);
    }
}
