use crossbeam::channel::Sender;
use crossterm::event::{Event, KeyCode};
use ratatui::{
    prelude::{Backend, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders},
};

use crate::ui::component::{Component, ComponentEvent, Direction, SideComponent};

pub struct ActionSelector<'c, B: Backend> {
    left: Option<SideComponent<'c, B>>,
    right: Option<SideComponent<'c, B>>,
    top: Option<SideComponent<'c, B>>,
    bottom: Option<SideComponent<'c, B>>,
    is_focused: bool,
}

impl<'c, B: Backend> ActionSelector<'c, B> {
    pub fn new() -> Self {
        Self {
            left: None,
            right: None,
            top: None,
            bottom: None,
            is_focused: false,
        }
    }
}

impl<'c, B: Backend> Default for ActionSelector<'c, B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'c, B: Backend + 'c> Component<'c, B> for ActionSelector<'c, B> {
    fn draw(&mut self, f: &mut ratatui::Frame<B>, area: Rect) {
        let block_style = if self.is_focused {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        };

        f.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .style(block_style)
                .border_type(BorderType::Rounded),
            area,
        );
    }

    fn handle(&mut self, event: ComponentEvent, sender: Sender<ComponentEvent>) {
        match event {
            ComponentEvent::ComponentFocus() => {
                self.is_focused = true;
            }
            ComponentEvent::UIEvent(event) => match event {
                Event::Key(key) => match key.code {
                    KeyCode::Left => {}
                    KeyCode::Right => {
                        self.is_focused = false;
                        sender.send(ComponentEvent::ComponentFocusLost(Direction::Right));
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
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
