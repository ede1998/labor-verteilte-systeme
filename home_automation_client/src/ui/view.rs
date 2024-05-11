use std::collections::HashMap;

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

#[derive(Debug, Copy, Clone)]
pub enum Border {
    NoHighlight,
    Blue,
    Magenta,
}

impl Border {
    fn is_highlighted(self) -> bool {
        !matches!(self, Border::NoHighlight)
    }

    fn color(self) -> Color {
        match self {
            Border::NoHighlight => Color::default(),
            Border::Blue => Color::Blue,
            Border::Magenta => Color::Magenta,
        }
    }

    pub fn highlighted(self, highlight: bool) -> Self {
        if highlight {
            self
        } else {
            Self::NoHighlight
        }
    }

    pub fn untitled(self) -> Block<'static> {
        self.titled("")
    }

    pub fn titled(self, title: &str) -> Block<'_> {
        use crate::utility::ApplyIf;
        use ratatui::style::Style;
        use ratatui::widgets::BorderType;
        Block::bordered()
            .title(title)
            .apply_if(self.is_highlighted(), |b| {
                b.border_style(self.color())
                    .border_type(BorderType::Thick)
                    .title_style(Style::from(self.color()).bold())
            })
    }
}

pub trait TextAreaExt {
    fn initial() -> Self;
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

    fn initial() -> Self {
        let mut input = TextArea::default();
        input.set_cursor_line_style(Default::default());
        input
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum PayloadTabKind {
    UpdateFrequency,
    Light,
    AirConditioning,
}

impl PayloadTabKind {
    pub fn cycle(self, up: bool) -> Self {
        match (up, self) {
            // go downwards through enum
            (true, Self::UpdateFrequency) => Self::Light,
            (true, Self::Light) => Self::AirConditioning,
            (true, Self::AirConditioning) => Self::UpdateFrequency,
            // go upwards through enum
            (false, Self::UpdateFrequency) => Self::AirConditioning,
            (false, Self::Light) => Self::UpdateFrequency,
            (false, Self::AirConditioning) => Self::Light,
        }
    }

    pub fn all() -> [Self; 3] {
        [Self::UpdateFrequency, Self::Light, Self::AirConditioning]
    }
}

impl std::fmt::Display for PayloadTabKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let text = match self {
            Self::UpdateFrequency => "Update frequency (Hz)",
            Self::Light => "Light (%)",
            Self::AirConditioning => "Air conditioning (On/Off)",
        };
        f.write_str(text)
    }
}

impl From<PayloadTabKind> for usize {
    fn from(value: PayloadTabKind) -> Self {
        value as _
    }
}

impl TryFrom<usize> for PayloadTabKind {
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::all().get(value).ok_or(value).copied()
    }

    type Error = usize;
}

impl From<&PayloadTab> for PayloadTabKind {
    fn from(value: &PayloadTab) -> Self {
        match value {
            PayloadTab::UpdateFrequency(_) => Self::UpdateFrequency,
            PayloadTab::Light { .. } => Self::Light,
            PayloadTab::AirConditioning(_) => Self::AirConditioning,
        }
    }
}

impl From<PayloadTabKind> for PayloadTab {
    fn from(value: PayloadTabKind) -> Self {
        match value {
            PayloadTabKind::UpdateFrequency => Self::default(),
            PayloadTabKind::Light => Self::Light { brightness: 0.0 },
            PayloadTabKind::AirConditioning => {
                Self::AirConditioning(ListState::default().with_selected(Some(0)))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum PayloadTab {
    UpdateFrequency(TextArea<'static>),
    Light { brightness: f32 },
    AirConditioning(ListState),
}

impl Default for PayloadTab {
    fn default() -> Self {
        let mut text_area = TextArea::initial();
        text_area.set_cursor_style(Default::default());
        text_area.set_block(Border::Magenta.untitled());
        Self::UpdateFrequency(text_area)
    }
}

impl PayloadTab {
    pub fn index(&self) -> usize {
        PayloadTabKind::from(self).into()
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
        Self {
            input: TextArea::initial(),
            list: ListState::default(),
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
