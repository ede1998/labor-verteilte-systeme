use std::{collections::HashMap, mem::Discriminant};

use crossterm::event::Event;
use home_automation_common::EntityState;
use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Stylize as _},
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

pub trait TextAreaExt {
    fn toggle_focus(&mut self, focused: bool);
    fn text(&self) -> &str;
}

impl TextAreaExt for TextArea<'_> {
    fn toggle_focus(&mut self, focused: bool) {
        let cursor = if focused {
            Modifier::REVERSED
        } else {
            Modifier::empty()
        };
        self.set_cursor_style(cursor.into());
    }

    fn text(&self) -> &str {
        self.lines().first().map_or("", std::ops::Deref::deref)
    }
}

#[derive(Debug, Clone)]
pub enum PayloadTab {
    UpdateFrequency(TextArea<'static>),
    Light { brightness: f32 },
    AirConditioning(bool),
}

impl Default for PayloadTab {
    fn default() -> Self {
        let mut text_area = TextArea::default();
        text_area.set_cursor_line_style(Default::default());
        text_area.set_cursor_style(Default::default());
        text_area.set_block(Block::bordered().border_style(Color::Magenta));
        Self::UpdateFrequency(text_area)
    }
}

impl PayloadTab {
    fn all() -> &'static [Discriminant<Self>; 3] {
        use std::{mem::discriminant, sync::OnceLock};
        static ALL: OnceLock<[Discriminant<PayloadTab>; 3]> = OnceLock::new();
        ALL.get_or_init(|| {
            [
                discriminant(&Self::UpdateFrequency(TextArea::default())),
                discriminant(&Self::Light { brightness: 0.0 }),
                discriminant(&Self::AirConditioning(true)),
            ]
        })
    }

    fn title(d: Discriminant<Self>) -> &'static str {
        let [freq, light, ac] = Self::all();
        match d {
            _ if d == *freq => "Update frequency (Hz)",
            _ if d == *light => "Light (%)",
            _ if d == *ac => "Air conditioning (On/Off)",
            _ => "",
        }
    }

    pub fn titles() -> impl Iterator<Item = &'static str> {
        Self::all().iter().map(|d| Self::title(*d))
    }

    pub fn max() -> usize {
        Self::all().len() - 1
    }

    pub fn index(&self) -> usize {
        let result = match self {
            PayloadTab::UpdateFrequency(_) => 0,
            PayloadTab::Light { .. } => 1,
            PayloadTab::AirConditioning(_) => 2,
        };
        debug_assert_eq!(result, {
            use std::mem::discriminant;
            let this = discriminant(self);
            Self::all()
                .iter()
                .position(|d| *d == this)
                .expect("Failed to find discriminant")
        });

        result
    }

    pub fn from_index(index: usize) -> Option<Self> {
        use std::mem::discriminant;
        let result = match index {
            0 => Self::default(),
            1 => Self::Light { brightness: 0.0 },
            2 => Self::AirConditioning(false),
            _ => return None,
        };
        debug_assert_eq!(
            Some(&discriminant(&result)),
            Self::all().get(index)
        );
        Some(result)
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
                stage: &data.stage,
                tab: &mut data.tab,
            }),
        }
    }
}
