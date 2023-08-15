use ratatui::{
    prelude::{Buffer, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget},
};

pub struct StepsBar {}

impl StepsBar {
    pub fn new() -> Self {
        StepsBar {}
    }
}

impl Widget for StepsBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let steps = vec!["Bootloader", "Partitions", "Kernel"];
        let current_step = 0;

        let blk = Block::default()
            .borders(Borders::all())
            .border_type(BorderType::Thick)
            .border_style(Style::default().fg(Color::Rgb(92, 92, 114)))
            .padding(Padding::new(1, 1, 0, 0));

        let mut spans_step = vec![];

        for (i, step) in steps.iter().enumerate() {
            if i == current_step {
                spans_step.push(Span::styled(
                    format!("  {} [{}]", step, i + 1),
                    Style::default().fg(Color::Rgb(86, 142, 163)),
                ));
            } else {
                spans_step.push(Span::raw(format!("  {} [{}]", step, i + 1)));
            }
            spans_step.push(Span::raw("  |"));
        }

        Paragraph::new(Line::from(spans_step))
            .block(blk)
            .render(area, buf);
    }
}
