use ratatui::{
    prelude::{Buffer, Rect},
    widgets::{Block, BorderType, Borders, Widget},
};

pub struct ConfigSelector {}

impl ConfigSelector {
    pub fn new() -> Self {
        ConfigSelector {}
    }
}

impl Widget for ConfigSelector {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Block::default()
            .borders(Borders::all())
            .border_type(BorderType::Rounded)
            .render(area, buf);
    }
}
