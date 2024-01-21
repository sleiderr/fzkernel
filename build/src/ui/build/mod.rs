#![allow(clippy::too_many_lines)]
use std::{error::Error, io, thread};

use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::{Alignment, Backend, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{block, Block, LineGauge, Paragraph, Widget},
    Frame,
};

use crate::{
    components::build::{BuildBlueprint, BuildEvent},
    errors::BuildError,
    APP, BOOTLOADER_BUILD, IMAGE_DISK_BUILD, TERMINAL,
};

#[derive(Default)]
pub struct BuildUI {
    steps_count: usize,
    steps_finished: usize,
}

impl BuildUI {
    pub fn run(mut self) -> Result<(), Box<dyn Error>> {
        enable_raw_mode()?;

        let mut blueprint = BuildBlueprint::default();
        let mut boot_step = BOOTLOADER_BUILD.get().ok_or(BuildError(None))?.lock();
        let mut image_disk_step = IMAGE_DISK_BUILD.get().ok_or(BuildError(None))?.lock();
        blueprint.steps.push(&mut *boot_step);
        blueprint.steps.push(&mut *image_disk_step);
        self.steps_count = blueprint.steps_count();

        let receiver = blueprint.incoming.clone();
        let ui = thread::spawn(move || {
            loop {
                let mut term_guard = TERMINAL.get().expect("Failed to load terminal").lock();
                let term = &mut *term_guard;
                term.draw(|f| self.render_ui(f))?;

                match receiver.recv().unwrap() {
                    BuildEvent::StepFinished(msg, time) => {
                        self.steps_finished += 1;
                        term.insert_before(1, |buf| {
                            Paragraph::new(Line::from(vec![
                                Span::from("["),
                                Span::styled("✔", Style::default().fg(Color::LightGreen)),
                                Span::from("] "),
                                Span::styled(
                                    format!(
                                        "Building {msg} {} / {}",
                                        self.steps_finished, self.steps_count
                                    ),
                                    Style::default().add_modifier(Modifier::BOLD),
                                ),
                                Span::from(format!(" in {:.3} s", time as f64 / 1_000_000_f64)),
                            ]))
                            .render(buf.area, buf);
                        })?;
                    }
                    BuildEvent::Update(msg) => {
                        term.insert_before(1, |buf| {
                            Paragraph::new(Line::from(vec![Span::styled(
                                msg,
                                Style::default().add_modifier(Modifier::BOLD),
                            )]))
                            .render(buf.area, buf);
                        })?;
                    }
                    BuildEvent::StepFailed(msg, output) => {
                        term.insert_before(2, |buf| {
                                Paragraph::new(vec![Line::from(vec![
                                Span::from("["),
                                Span::styled("✗", Style::default().fg(Color::LightRed)),
                                Span::from("] "),
                                Span::styled(msg, Style::default().add_modifier(Modifier::BOLD)),
                            ]), Line::from(Span::styled(
                                    "    Get more information on the error by using the -v argument",
                                    Style::default().fg(Color::LightYellow)
                                ))])
                                .render(buf.area, buf);
                            })?;

                        if APP.get().unwrap().lock().verbose {
                            term.insert_before(2, |buf| {
                                Paragraph::new(vec![
                                    Line::from("\n"),
                                    Line::from(Span::styled(
                                        "➤ Cargo output: ",
                                        Style::default()
                                            .add_modifier(Modifier::BOLD)
                                            .fg(Color::LightMagenta),
                                    )),
                                ])
                                .render(buf.area, buf);
                            })?;
                            let lines: Vec<Line> = output
                                .split('\n')
                                .map(|line| {
                                    Line::from(Span::styled(
                                        line,
                                        Style::default().add_modifier(Modifier::ITALIC),
                                    ))
                                })
                                .collect();
                            for lines_chk in
                                lines.rchunks(term.size().unwrap().height as usize).rev()
                            {
                                term.insert_before(term.size().unwrap().height, |buf| {
                                    Paragraph::new(lines_chk.to_vec()).render(buf.area, buf);
                                })?;
                            }
                        };
                        break;
                    }
                    BuildEvent::Finished(_, _) => break,
                    _ => {}
                }
            }
            disable_raw_mode()?;
            let mut term_guard = TERMINAL.get().expect("Failed to load terminal").lock();
            let term = &mut *term_guard;

            execute!(term.backend_mut(), DisableMouseCapture)?;
            term.show_cursor()?;

            Ok::<(), io::Error>(())
        });
        futures::executor::block_on(blueprint.build())?;
        ui.join();

        Ok(())
    }

    fn render_ui<B: Backend>(&mut self, f: &mut Frame<B>) {
        let size = f.size();

        let block =
            Block::default().title(block::Title::from("Progress").alignment(Alignment::Center));
        f.render_widget(block, size);

        let chunks = Layout::default()
            .constraints(vec![Constraint::Length(2), Constraint::Length(4)])
            .margin(1)
            .split(size);

        // total progress
        let progress = LineGauge::default()
            .gauge_style(Style::default().fg(Color::Blue))
            .label(format!("{}/{}", self.steps_finished, self.steps_count))
            .ratio(self.steps_finished as f64 / self.steps_count as f64);
        f.render_widget(progress, chunks[0]);
    }
}
