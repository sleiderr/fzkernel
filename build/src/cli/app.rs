use std::{cell::RefCell, io, rc::Rc};

use argh::FromArgs;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    prelude::{Backend, Constraint, Direction, Layout},
    Terminal,
};

use crate::ui::{
    action_selector::ActionSelector,
    component::{Component, ComponentManager},
    config::ConfigSelector,
    footer::Footer,
    main::draw_ui,
    steps::StepsBar,
};

#[derive(FromArgs)]
#[argh(description = "FrozenBoot build helper")]
pub struct App {
    #[argh(
        switch,
        short = 's',
        description = "build FrozenBoot in standalone mode"
    )]
    pub standalone: bool,

    #[argh(
        switch,
        short = 'f',
        description = "fast build using default parameters"
    )]
    pub fast: bool,

    #[argh(switch, short = 'v', description = "display debug messages")]
    pub verbose: bool,
}

pub fn run_app<B: Backend + 'static>(term: &mut Terminal<B>) -> io::Result<()> {
    let main_vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Max(1), Constraint::Max(1)])
        .split(term.size()?);

    let main_horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(main_vertical[0]);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(main_horizontal[1]);

    let footer = Footer::default();
    let footer_ptr = Rc::new(RefCell::new(footer));
    let action_selector = ActionSelector::default();
    let action_ptr = Rc::new(RefCell::new(action_selector));
    let steps = StepsBar::default();
    let steps_ptr = Rc::new(RefCell::new(steps));
    let config = ConfigSelector::new();
    let config_ptr = Rc::new(RefCell::new(config));

    config_ptr
        .borrow_mut()
        .set_layout(Some(action_ptr.clone()), None, None, None);
    action_ptr
        .borrow_mut()
        .set_layout(None, Some(config_ptr.clone()), None, None);

    let mut cm: ComponentManager<B> = ComponentManager::from_component(config_ptr, layout[1]);
    cm.add(action_ptr, main_horizontal[0]);
    cm.add(steps_ptr, layout[0]);
    cm.add(footer_ptr, main_vertical[2]);
    cm.run(term);

    Ok(())
}
