use crossbeam::channel::Sender;
use ratatui::{
    prelude::{Backend, Buffer, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget},
};

use crate::ui::component::{Component, ComponentEvent};

#[derive(Default)]
pub struct StepsBar {
    tab: usize,
}

impl<'c, B: Backend + 'c> Component<'c, B> for StepsBar {
    fn draw(&mut self, f: &mut ratatui::Frame<B>, area: Rect) {
        let steps = ["Bootloader", "Partitions", "Kernel"];
        let current_step = 0;

        let blk = Block::default()
            .borders(Borders::all())
            .border_type(BorderType::Thick)
            .border_style(Style::default().fg(Color::Rgb(92, 92, 114)))
            .padding(Padding::new(1, 1, 0, 0));

        let mut spans_step = vec![];

        for (i, step) in steps.iter().enumerate() {
            if i == self.tab {
                spans_step.push(Span::styled(
                    format!("  {} [{}]", step, i + 1),
                    Style::default().fg(Color::Rgb(86, 142, 163)),
                ));
            } else {
                spans_step.push(Span::raw(format!("  {} [{}]", step, i + 1)));
            }
            spans_step.push(Span::raw("  |"));
        }

        f.render_widget(Paragraph::new(Line::from(spans_step)).block(blk), area);
    }

    fn handle(&mut self, event: ComponentEvent, sender: Sender<ComponentEvent>) {
        match event {
            ComponentEvent::TabSwitch(new_tab) => {
                self.tab = new_tab;
            }
            _ => {}
        }
    }

    fn left(&self) -> Option<super::component::SideComponent<'c, B>> {
        todo!()
    }

    fn right(&self) -> Option<super::component::SideComponent<'c, B>> {
        todo!()
    }

    fn top(&self) -> Option<super::component::SideComponent<'c, B>> {
        todo!()
    }

    fn bottom(&self) -> Option<super::component::SideComponent<'c, B>> {
        todo!()
    }

    fn set_layout(
        &mut self,
        left: Option<super::component::SideComponent<'c, B>>,
        right: Option<super::component::SideComponent<'c, B>>,
        top: Option<super::component::SideComponent<'c, B>>,
        bottom: Option<super::component::SideComponent<'c, B>>,
    ) {
        todo!()
    }
}
