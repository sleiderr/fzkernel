use ratatui::{
    prelude::{Buffer, Rect},
    widgets::{Block, BorderType, Borders, Widget},
};

pub struct ActionSelector {}

impl ActionSelector {
    pub fn new() -> Self {
        ActionSelector {}
    }
}

impl Widget for ActionSelector {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .render(area, buf);
    }
}
