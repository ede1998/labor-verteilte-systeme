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

#[derive(Debug, Clone, Default)]
pub enum PayloadTab {
    #[default]
    UpdateFrequency,
    Light,
    AirConditioning,
}

impl std::fmt::Display for PayloadTab {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let text = match self {
            PayloadTab::UpdateFrequency => "Update frequency",
            PayloadTab::Light => "Light",
            PayloadTab::AirConditioning => "Air conditioning",
        };
        f.write_str(text)
    }
}

impl PayloadTab {
    pub fn iter() -> impl Iterator<Item = Self> {
        [Self::UpdateFrequency, Self::Light, Self::AirConditioning].into_iter()
    }
}

#[derive(Debug, Clone)]
pub enum SendStage {
    EntitySelect,
    PayloadSelect {},
}

#[derive(Debug, Clone)]
pub struct SendData {
    pub input: TextArea<'static>,
    pub list: ListState,
    pub stage: SendStage,
    pub tab: PayloadTab,
}

impl Default for SendData {
    fn default() -> Self {
        let list = ListState::default();
        let mut input = TextArea::default();
        input.set_cursor_line_style(Default::default());
        Self {
            input,
            list,
            stage: SendStage::EntitySelect,
            tab: Default::default(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub enum View {
    #[default]
    Monitor,
    Send(SendData),
}

impl View {
    pub fn ensure_send_mut(&mut self) -> &mut SendData {
        loop {
            match self {
                View::Monitor => {
                    *self = View::Send(Default::default());
                }
                View::Send(data) => break data,
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
            Self::Send(data) => Views::SendView(SendView {
                state,
                entity_input: &mut data.input,
                list: &mut data.list,
                stage: &mut data.stage,
            }),
        }
    }
}
