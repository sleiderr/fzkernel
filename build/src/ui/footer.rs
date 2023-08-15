use ratatui::{
    layout,
    prelude::{Buffer, Constraint, Direction, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

pub struct Footer {}

impl Footer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Widget for Footer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        const VERSION: &str = env!("CARGO_PKG_VERSION");

        let chunks = layout::Layout::new()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(46),
                Constraint::Percentage(37),
                Constraint::Percentage(20),
            ])
            .split(area);

        Block::default()
            .style(Style::default().bg(Color::Rgb(52, 52, 74)))
            .render(area, buf);

        let mut spans_status = vec![];
        spans_status.push(Span::styled(
            "  BUILD  ",
            Style::default()
                .fg(Color::Rgb(200, 200, 200))
                .bg(Color::Rgb(86, 142, 163)),
        ));
        let par = Paragraph::new(Line::from(spans_status)).render(chunks[0], buf);

        let mut spans_name = vec![];
        spans_name.push(Span::raw("FrozenBoot "));
        spans_name.push(Span::raw(VERSION));
        let km_par = Paragraph::new(Line::from(spans_name)).render(chunks[1], buf);

        let mut spans_keymap = vec![];
        spans_keymap.push(Span::raw("[n]ext  "));
        spans_keymap.push(Span::raw("[p]revious  "));
        spans_keymap.push(Span::raw("[q]uit  "));
        Paragraph::new(Line::from(spans_keymap)).render(chunks[2], buf);
    }
}
