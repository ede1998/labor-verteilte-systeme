use std::collections::HashMap;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use home_automation_common::EntityState;
use ratatui::{
    prelude::*,
    widgets::{block::Title, Block, List, ListState},
};
use tui_textarea::TextArea;

use crate::{ui::app::Action, utility::ApplyIf as _};

use super::{prepare_scaffolding, SendStage, UiView, View};

pub struct SendView<'a> {
    pub(super) state: &'a HashMap<String, EntityState>,
    pub(super) entity_input: &'a TextArea<'a>,
    pub(super) list: &'a mut ListState,
    pub(super) stage: &'a SendStage,
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
        let entity_focussed = matches!(self.stage, SendStage::EntitySelect);
        let list_focussed = self.list.selected().is_some();

        let container = block("Entity", entity_focussed, Color::Blue);
        frame.render_widget(&container, area);

        let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]);
        let [input_area, list_area] = *layout.split(container.inner(area)) else {
            panic!("Failed to setup layout");
        };

        let input_block = block("", entity_focussed && !list_focussed, Color::Magenta);

        let list = List::new(self.state.keys().map(Span::raw))
            .block(block("", entity_focussed && list_focussed, Color::Magenta))
            // invert color scheme for selected line
            .highlight_style(Modifier::REVERSED);

        frame.render_widget(&input_block, input_area);
        frame.render_widget(self.entity_input.widget(), input_block.inner(input_area));
        frame.render_stateful_widget(list, list_area, self.list);
    }

    fn render_payload_select(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::Tabs;

        let payload_selection_active = matches!(self.stage, SendStage::PayloadSelect { .. });

        let tabs = Tabs::new(vec!["TODO", "IN PROGRESS", "DONE"])
            .block(block("Payload", payload_selection_active, Color::Blue))
            .style(Style::default().white())
            .highlight_style(Style::default().underlined().bold().yellow())
            .select(1)
            .divider(symbols::DOT)
            .padding(" ", " ");

        frame.render_widget(tabs, area);
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
        let update_selection_index = |increase| {
            let max = self.state.len().checked_sub(1)?;
            let current = self.list.selected()?;
            match increase {
                true if current >= max => Some(0),
                false if current == 0 => Some(max),
                true => Some(current + 1),
                false => Some(current - 1),
            }
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
            }) => Some(Action::SetRecipientSelection(update_selection_index(true))),
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                kind: KeyEventKind::Press,
                ..
            }) => Some(Action::SetRecipientSelection(update_selection_index(false))),
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
        let [name_area, payload_area] = *outer_layout.split(block.inner(frame.size())) else {
            panic!("Failed to setup layout.")
        };
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
