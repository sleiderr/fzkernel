use crossbeam::channel::Sender;
use ratatui::{
    layout,
    prelude::{Backend, Buffer, Constraint, Direction, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use crate::ui::component::{Component, ComponentEvent, SideComponent};

pub struct Footer<'c, B> {
    left: Option<SideComponent<'c, B>>,
    right: Option<SideComponent<'c, B>>,
    top: Option<SideComponent<'c, B>>,
    bottom: Option<SideComponent<'c, B>>,
}

impl<'c, B> Footer<'c, B> {
    pub fn new() -> Self {
        Self {
            left: None,
            right: None,
            top: None,
            bottom: None,
        }
    }
}

impl<'c, B: Backend> Default for Footer<'c, B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'c, B: Backend + 'c> Component<'c, B> for Footer<'c, B> {
    fn handle(&mut self, event: ComponentEvent, sender: Sender<ComponentEvent>) {
        match event {
            _ => {}
        }
    }

    fn draw(&mut self, f: &mut ratatui::Frame<B>, area: Rect) {
        const VERSION: &str = env!("CARGO_PKG_VERSION");

        let chunks = layout::Layout::new()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(46),
                Constraint::Percentage(37),
                Constraint::Percentage(20),
            ])
            .split(area);

        f.render_widget(
            Block::default().style(Style::default().bg(Color::Rgb(52, 52, 74))),
            area,
        );

        let spans_status = vec![Span::styled(
            "  BUILD  ",
            Style::default()
                .fg(Color::Rgb(200, 200, 200))
                .bg(Color::Rgb(86, 142, 163)),
        )];
        f.render_widget(Paragraph::new(Line::from(spans_status)), chunks[0]);

        let spans_name = vec![Span::raw("FrozenBoot "), Span::raw(VERSION)];
        f.render_widget(Paragraph::new(Line::from(spans_name)), chunks[1]);

        let spans_keymap = vec![
            Span::raw("[n]ext  "),
            Span::raw("[p]revious  "),
            Span::raw("[q]uit  "),
        ];
        f.render_widget(Paragraph::new(Line::from(spans_keymap)), chunks[2]);
    }

    fn left(&self) -> Option<SideComponent<'c, B>> {
        let left = self.left.clone()?;
        Some(left)
    }

    fn right(&self) -> Option<SideComponent<'c, B>> {
        let right = self.right.clone()?;
        Some(right)
    }

    fn top(&self) -> Option<SideComponent<'c, B>> {
        let top = self.top.clone()?;
        Some(top)
    }

    fn bottom(&self) -> Option<SideComponent<'c, B>> {
        let bottom = self.bottom.clone()?;
        Some(bottom.clone())
    }

    fn set_layout(
        &mut self,
        left: Option<SideComponent<'c, B>>,
        right: Option<SideComponent<'c, B>>,
        top: Option<SideComponent<'c, B>>,
        bottom: Option<SideComponent<'c, B>>,
    ) {
        self.left = left;
        self.right = right;
        self.top = top;
        self.bottom = bottom;
    }
}
