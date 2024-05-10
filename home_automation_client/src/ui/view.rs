use std::collections::HashMap;

use crossterm::event::Event;
use home_automation_common::EntityState;
use ratatui::{
    layout::Alignment,
    style::Stylize as _,
    symbols::border,
    widgets::{
        block::{Position, Title},
        Block, Borders, ListState,
    },
    Frame,
};
use tui_textarea::TextArea;

use super::app::Action;

mod monitor;
mod send;

pub use monitor::MonitorView;
pub use send::SendView;

pub trait UiView {
    fn handle_events(&self, event: Event) -> Option<Action>;
    fn render(&mut self, frame: &mut Frame);
}

fn prepare_scaffolding(instructions: Title) -> Block {
    let title = Title::from(" Home Automation Client ".bold());
    Block::default()
        .title(title.alignment(Alignment::Center))
        .title(
            instructions
                .alignment(Alignment::Center)
                .position(Position::Bottom),
        )
        .borders(Borders::ALL)
        .border_set(border::THICK)
}

#[derive(Debug, Clone)]
pub enum SendStage {
    EntitySelect,
    PayloadSelect {},
}

#[derive(Debug, Default, Clone)]
pub enum View {
    #[default]
    Monitor,
    Send {
        input: TextArea<'static>,
        list: ListState,
        stage: SendStage,
    },
}

impl View {
    pub fn send() -> Self {
        let list = ListState::default();
        let mut input = TextArea::default();
        input.set_cursor_line_style(Default::default());
        Self::Send {
            input,
            list,
            stage: SendStage::EntitySelect,
        }
    }

    pub fn ensure_send_mut(&mut self) -> (&mut TextArea<'static>, &mut ListState, &mut SendStage) {
        loop {
            match self {
                View::Monitor => {
                    *self = View::send();
                }
                View::Send { input, list, stage } => break (input, list, stage),
            }
        }
    }

    pub fn active<'a>(&'a mut self, state: &'a HashMap<String, EntityState>) -> impl UiView + 'a {
        macro_rules! all_views {
            ($($view:ident),+) => {
                enum Views<'b> {
                    $($view($view<'b>),)+
                }
                impl<'b> UiView for Views<'b> {
                    fn handle_events(&self, event: crossterm::event::Event) -> Option<Action> {
                        match self {
                            $(Self::$view(v) => v.handle_events(event),)+
                        }
                    }

                    fn render(&mut self, frame: &mut ratatui::Frame) {
                        match self {
                            $(Self::$view(v) => v.render(frame),)+
                        }
                    }
                }
            };
        }
        all_views!(MonitorView, SendView);

        match self {
            Self::Monitor => Views::MonitorView(MonitorView(state)),
            Self::Send { input, list, stage } => Views::SendView(SendView {
                state,
                entity_input: input,
                list,
                stage,
            }),
        }
    }
}
