use std::{cell::RefCell, rc::Rc, time::Duration};

use crossbeam::channel::{unbounded, Receiver, Sender};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    prelude::{Backend, Rect},
    Frame, Terminal,
};

pub type SideComponent<'c, B> = Rc<RefCell<dyn Component<'c, B>>>;

#[derive(Clone, Copy)]
pub enum Direction {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone)]
pub enum ComponentEvent {
    TabRequest,
    TabSwitch(usize),
    ComponentFocus(),
    ComponentFocusLost(Direction),
    UIEvent(Event),
    InputLock,
}

pub trait Component<'c, B: Backend> {
    fn draw(&mut self, f: &mut Frame<B>, area: Rect);
    fn left(&self) -> Option<SideComponent<'c, B>>;
    fn right(&self) -> Option<SideComponent<'c, B>>;
    fn top(&self) -> Option<SideComponent<'c, B>>;
    fn bottom(&self) -> Option<SideComponent<'c, B>>;
    fn handle(&mut self, event: ComponentEvent, sender: Sender<ComponentEvent>);
    fn set_layout(
        &mut self,
        left: Option<SideComponent<'c, B>>,
        right: Option<SideComponent<'c, B>>,
        top: Option<SideComponent<'c, B>>,
        bottom: Option<SideComponent<'c, B>>,
    );
}

pub struct ComponentManager<'cm, B: Backend> {
    current_component: SideComponent<'cm, B>,
    components_table: Vec<(Rect, SideComponent<'cm, B>)>,
    sender: Sender<ComponentEvent>,
    receiver: Receiver<ComponentEvent>,
    input_locked: bool,
    current_tab: isize,
    tab_count: usize,
}

impl<'cm, B: Backend> ComponentManager<'cm, B> {
    pub fn run(&mut self, term: &mut Terminal<B>) {
        loop {
            term.draw(|f| self.render(f));

            if event::poll(Duration::from_secs(0)).unwrap() {
                match event::read().unwrap() {
                    Event::Key(key) => {
                        if self.input_locked {
                            self.current_component.borrow_mut().handle(
                                ComponentEvent::UIEvent(Event::Key(key)),
                                self.sender.clone(),
                            );
                        } else {
                            match key.code {
                                KeyCode::Char('q') => {
                                    break;
                                }
                                KeyCode::Char('n') => {
                                    self.current_tab =
                                        (self.current_tab + 1) % (self.tab_count as isize - 1);
                                    self.sender
                                        .send(ComponentEvent::TabSwitch(self.current_tab as usize));
                                }
                                KeyCode::Char('p') => {
                                    self.current_tab =
                                        (self.current_tab - 1) % (self.tab_count as isize - 1);
                                    self.sender.send(ComponentEvent::TabSwitch(
                                        self.current_tab.unsigned_abs(),
                                    ));
                                }
                                _ => self.current_component.borrow_mut().handle(
                                    ComponentEvent::UIEvent(Event::Key(key)),
                                    self.sender.clone(),
                                ),
                            }
                        }
                    }
                    _ => {}
                }
            }

            self.poll_component_event();
        }
    }

    pub fn key_event(&mut self, event: KeyEvent) {}

    pub fn broadcast(&mut self, event: ComponentEvent) {
        for (_, component) in &self.components_table {
            component
                .borrow_mut()
                .handle(event.clone(), self.sender.clone());
        }
    }

    pub fn poll_component_event(&mut self) {
        match self.receiver.try_recv().ok() {
            Some(ComponentEvent::TabSwitch(usize)) => {
                self.broadcast(ComponentEvent::TabSwitch(usize))
            }
            Some(ComponentEvent::InputLock) => {
                self.input_locked = !self.input_locked;
            }
            Some(ComponentEvent::ComponentFocusLost(dir)) => match dir {
                Direction::Left => {
                    let left_component = self.current_component.borrow().left();
                    if left_component.is_none() {
                        return;
                    }
                    self.current_component = left_component.unwrap().clone();
                    self.current_component
                        .borrow_mut()
                        .handle(ComponentEvent::ComponentFocus(), self.sender.clone());
                }
                Direction::Right => {
                    let right_component = self.current_component.borrow().right();
                    if right_component.is_none() {
                        return;
                    }
                    self.current_component = right_component.unwrap().clone();
                    self.current_component
                        .borrow_mut()
                        .handle(ComponentEvent::ComponentFocus(), self.sender.clone());
                }
                Direction::Top => {
                    let top_component = self.current_component.borrow().top();
                    if top_component.is_none() {
                        return;
                    }
                    self.current_component = top_component.unwrap().clone();
                    self.current_component
                        .borrow_mut()
                        .handle(ComponentEvent::ComponentFocus(), self.sender.clone());
                }
                Direction::Bottom => {
                    let bottom_component = self.current_component.borrow().bottom();
                    if bottom_component.is_none() {
                        return;
                    }
                    self.current_component = bottom_component.unwrap().clone();
                    self.current_component
                        .borrow_mut()
                        .handle(ComponentEvent::ComponentFocus(), self.sender.clone());
                }
            },
            _ => {}
        }
    }

    pub fn from_component(component: SideComponent<'cm, B>, area: Rect) -> Self {
        let (sender, receiver) = unbounded();
        let table = vec![(area, component.clone())];

        Self {
            current_component: component.clone(),
            components_table: table,
            sender,
            receiver,
            input_locked: false,
            current_tab: 0,
            tab_count: 4,
        }
    }

    pub fn add(&mut self, component: SideComponent<'cm, B>, area: Rect) {
        self.components_table.push((area, component));
    }

    pub fn render(&mut self, f: &mut Frame<B>) {
        for (area, component) in self.components_table.iter_mut() {
            component.borrow_mut().draw(f, *area);
        }
    }
}
