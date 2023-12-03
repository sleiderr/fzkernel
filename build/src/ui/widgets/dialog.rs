use ratatui::{
    prelude::{self, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, List, Padding, Paragraph, Widget},
};

#[derive(Clone, Copy)]
pub struct DialogBox<'db> {
    message: &'db str,
    title: &'db str,
}

impl<'db> DialogBox<'db> {
    pub fn new(title: &'db str, message: &'db str) -> Self {
        Self { message, title }
    }

    pub fn area(&self, base: Rect) -> Rect {
        self.draw_box(base, 55, 35)
    }

    fn draw_box(&self, area: Rect, x_size: u16, y_size: u16) -> Rect {
        let dialog_box = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - y_size) / 2),
                Constraint::Percentage(y_size),
                Constraint::Percentage((100 - y_size) / 2),
            ])
            .split(area);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - x_size) / 2),
                Constraint::Percentage(x_size),
                Constraint::Percentage((100 - x_size) / 2),
            ])
            .split(dialog_box[1])[1]
    }
}

impl<'db> Widget for DialogBox<'db> {
    fn render(self, area: Rect, buf: &mut prelude::Buffer) {
        let box_body = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::new(2, 2, 2, 2))
            .style(Style::default().bg(Color::Rgb(0, 0, 150)));
        let box_container = self.draw_box(area, 55, 35);
        let dialog_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Max(5)])
            .split(box_container);
        let text_container = Block::default().padding(Padding::new(4, 4, 4, 4));
        let content = Paragraph::new(Line::from(self.message)).block(text_container);

        let input_container = Block::default().padding(Padding::new(4, 2, 2, 2));
        let input = Paragraph::new(Line::from("test")).block(input_container);
        List;
        box_body.render(box_container, buf);
        content.render(dialog_layout[0], buf);
        input.render(dialog_layout[1], buf);
    }
}
