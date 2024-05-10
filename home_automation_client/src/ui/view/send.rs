use std::collections::HashMap;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use home_automation_common::EntityState;
use ratatui::{
    prelude::*,
    widgets::{block::Title, Block, List, ListState},
};
use tui_textarea::TextArea;

use crate::{
    ui::{app::Action, view::PayloadTab},
    utility::{ApplyIf as _, Wrapping},
};

use super::{prepare_scaffolding, toggle_focus, SendStage, UiView, View};

pub struct SendView<'a> {
    pub(super) state: &'a HashMap<String, EntityState>,
    pub(super) entity_input: &'a mut TextArea<'static>,
    pub(super) list: &'a mut ListState,
    pub(super) stage: &'a SendStage,
    pub(super) tab: &'a mut PayloadTab,
}

fn block(title: &str, highlighted: bool, color: Color) -> Block {
    use ratatui::widgets::BorderType;
    Block::bordered().title(title).apply_if(highlighted, |b| {
        b.border_style(color)
            .border_type(BorderType::Thick)
            .title_style(Style::from(color).bold())
    })
}

impl<'a> SendView<'a> {
    fn render_name_select(&mut self, frame: &mut Frame, area: Rect) {
        let entity_focused = matches!(self.stage, SendStage::EntitySelect);
        let list_focused = entity_focused && self.list.selected().is_some();

        let container = block("Entity", entity_focused, Color::Blue);
        frame.render_widget(&container, area);

        let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]);
        let [input_area, list_area] = layout.areas(container.inner(area));

        let input_block = block("", !list_focused, Color::Magenta);
        toggle_focus(entity_focused && !list_focused, self.entity_input);

        let list = List::new(self.state.keys().map(Span::raw))
            .block(block("", list_focused, Color::Magenta))
            // invert color scheme for selected line
            .highlight_style(Modifier::REVERSED);

        frame.render_widget(&input_block, input_area);
        frame.render_widget(self.entity_input.widget(), input_block.inner(input_area));
        frame.render_stateful_widget(list, list_area, self.list);
    }

    fn render_payload_select(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::Tabs;

        let payload_selection_active = matches!(self.stage, SendStage::PayloadSelect { .. });

        let container = block("Payload", payload_selection_active, Color::Blue);
        frame.render_widget(&container, area);

        let layout = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]);
        let [tab_header_area, tab_content_area] = layout.areas(container.inner(area));

        let tabs = Tabs::new(PayloadTab::iter().map(|t| t.to_string()))
            .highlight_style(Style::from(Color::Magenta).bold());

        match self.tab {
            PayloadTab::UpdateFrequency(text) => {
                toggle_focus(payload_selection_active, text);
                let layout = Layout::vertical([Constraint::Length(3)]);
                let [area] = layout.areas(tab_content_area);
                frame.render_widget(text.widget(), area);
            }
            PayloadTab::Light { brightness } => todo!(),
            PayloadTab::AirConditioning(_) => todo!(),
        }

        frame.render_widget(tabs, tab_header_area);
    }

    fn handle_generic_event(&self, event: &Event) -> Option<Action> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) => Some(Action::ChangeView(View::Monitor)),
            _ => None,
        }
    }

    fn handle_name_select_event(&self, event: &Event) -> Option<Action> {
        let update_index = |increase: fn(Wrapping) -> Wrapping| {
            let current = self.list.selected()?;
            let max = self.state.len().checked_sub(1)?;
            Some(increase(Wrapping::new(current, max)).current())
        };
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            }) => {
                let recipient = match self.list.selected() {
                    Some(index) => self.state.keys().nth(index)?,
                    None => self.entity_input.lines().first()?,
                };
                Some(Action::SetMessageRecipient(recipient.to_owned()))
            }
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                kind: KeyEventKind::Press,
                ..
            }) if !self.state.is_empty() => {
                let inverted_selection = self.list.selected().xor(Some(0));
                Some(Action::SetRecipientSelection(inverted_selection))
            }
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                kind: KeyEventKind::Press,
                ..
            }) => Some(Action::SetRecipientSelection(update_index(Wrapping::inc))),
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                kind: KeyEventKind::Press,
                ..
            }) => Some(Action::SetRecipientSelection(update_index(Wrapping::dec))),
            event if self.list.selected().is_none() => {
                Some(Action::TextInput(event.clone().into()))
            }
            _ => None,
        }
    }
}

impl<'a> UiView for SendView<'a> {
    fn render(&mut self, frame: &mut Frame) {
        let instructions = Title::from(Line::from(vec![
            " Accept input".into(),
            "<ENTER>".blue().bold(),
            " Switch focus ".into(),
            "<TAB>".blue().bold(),
            " Select ".into(),
            "<UP>/<DOWN>/<LEFT>/<RIGHT>".blue().bold(),
            " Abort ".into(),
            "<ESC> ".blue().bold(),
        ]));
        let block = prepare_scaffolding(instructions)
            .title(Title::from("Send Message".bold()).alignment(Alignment::Left));
        frame.render_widget(&block, frame.size());

        let outer_layout = Layout::vertical([Constraint::Min(10), Constraint::Min(10)]);
        let [name_area, payload_area] = outer_layout.areas(block.inner(frame.size()));
        self.render_name_select(frame, name_area);
        self.render_payload_select(frame, payload_area);
    }

    fn handle_events(&self, event: Event) -> Option<Action> {
        self.handle_generic_event(&event)
            .or_else(|| match self.stage {
                SendStage::EntitySelect => self.handle_name_select_event(&event),
                SendStage::PayloadSelect {} => todo!(),
            })
    }
}