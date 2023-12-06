use ratatui::{
    prelude::{Backend, Constraint, Direction, Layout},
    Frame,
};

use crate::ui::{
    action_selector::ActionSelector, config::ConfigSelector, footer::Footer, steps::StepsBar,
};

pub fn draw_ui<B: Backend>(f: &mut Frame<B>) {
    let main_vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Max(1), Constraint::Max(1)])
        .split(f.size());

    let main_horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(main_vertical[0]);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(main_horizontal[1]);
}
