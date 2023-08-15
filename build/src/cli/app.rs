use std::io;

use argh::FromArgs;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{prelude::Backend, Terminal};

use crate::ui::main::draw_ui;

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

pub fn run_app<B: Backend>(term: &mut Terminal<B>) -> io::Result<()> {
    loop {
        term.draw(|f| draw_ui(f))?;

        match event::read()? {
            Event::Key(key) => match key.code {
                KeyCode::Char(ch) => {
                    if ch == 'q' {
                        return Ok(());
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}
