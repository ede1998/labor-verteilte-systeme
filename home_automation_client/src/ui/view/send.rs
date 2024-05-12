use std::collections::HashMap;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use home_automation_common::{
    protobuf::{ActuatorState, NamedEntityState},
    EntityState,
};
use ratatui::{
    prelude::*,
    widgets::{block::Title, List, ListState},
};
use tui_textarea::TextArea;

use crate::{
    ui::{
        app::Action,
        view::{PayloadTab, PayloadTabKind},
    },
    utility::{ApplyIf as _, HashMapExt, Wrapping},
};

use super::{prepare_scaffolding, Border, SendStage, TextAreaExt, UiView, View};

pub struct SendView<'a> {
    pub(super) state: &'a HashMap<String, EntityState>,
    pub(super) entity_input: &'a mut TextArea<'static>,
    pub(super) list: &'a mut ListState,
    pub(super) stage: &'a SendStage,
    pub(super) tab: &'a mut PayloadTab,
}

impl<'a> SendView<'a> {
    fn render_name_select(&mut self, frame: &mut Frame, area: Rect) {
        let entity_focused = matches!(self.stage, SendStage::EntitySelect);
        let list_focused = entity_focused && self.list.selected().is_some();

        let container = Border::Blue.highlighted(entity_focused).titled("Entity");
        frame.render_widget(&container, area);

        let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]);
        let [input_area, list_area] = layout.areas(container.inner(area));

        let input_block = Border::Magenta.highlighted(!list_focused).untitled();
        self.entity_input
            .toggle_focus(entity_focused && !list_focused);

        let list = List::new(self.state.keys_stable().map(Span::raw))
            .block(Border::Magenta.highlighted(list_focused).untitled())
            // invert color scheme for selected line
            .highlight_style(Modifier::REVERSED);

        frame.render_widget(&input_block, input_area);
        frame.render_widget(self.entity_input.widget(), input_block.inner(input_area));
        frame.render_stateful_widget(list, list_area, self.list);
    }

    fn render_payload_select(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Gauge, Tabs};

        let payload_selection_active = matches!(self.stage, SendStage::PayloadSelect { .. });

        let container = Border::Blue
            .highlighted(payload_selection_active)
            .titled("Payload");
        frame.render_widget(&container, area);

        let layout = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]);
        let [tab_header_area, tab_content_area] = layout.areas(container.inner(area));

        let allowed_payloads = self.determine_allowed_payload_tabs();
        let tabs = Tabs::new(PayloadTabKind::all().map(|t| {
            Span::raw(t.to_string()).apply_if(allowed_payloads.contains(&t), |s| {
                s.style(Modifier::UNDERLINED)
            })
        }))
        .highlight_style(Style::from(Color::Magenta).bold())
        .select(self.tab.index());

        match self.tab {
            PayloadTab::UpdateFrequency(text) => {
                text.toggle_focus(payload_selection_active);
                let layout = Layout::vertical([Constraint::Length(3)]);
                let [area] = layout.areas(tab_content_area);
                frame.render_widget(text.widget(), area);
            }
            PayloadTab::Light { brightness } => {
                let layout = Layout::vertical([Constraint::Length(5)]);
                let [area] = layout.areas(tab_content_area);
                let brightness = f64::from(*brightness);
                let gauge = Gauge::default()
                    .block(Border::Magenta.untitled())
                    .gauge_style(Color::Magenta)
                    .ratio(brightness / 100.0)
                    .label(format!("{brightness:.1}%"))
                    .use_unicode(true);
                frame.render_widget(gauge, area);
            }
            PayloadTab::AirConditioning(state) => {
                let layout = Layout::vertical([Constraint::Length(4)]);
                let [area] = layout.areas(tab_content_area);
                let list = List::new(["On", "Off"])
                    .block(Border::Magenta.untitled())
                    // invert color scheme for selected line
                    .highlight_style(Modifier::REVERSED);
                frame.render_stateful_widget(list, area, state);
            }
        }

        frame.render_widget(tabs, tab_header_area);
    }

    fn determine_allowed_payload_tabs(&self) -> Vec<PayloadTabKind> {
        use home_automation_common::protobuf::{actuator_state::State, ActuatorState};
        let entity_name = self.entity_input.text();
        match self.state.get(entity_name) {
            Some(EntityState::Actuator(ActuatorState {
                state: Some(State::AirConditioning(_)),
            })) => vec![
                PayloadTabKind::UpdateFrequency,
                PayloadTabKind::AirConditioning,
            ],
            Some(EntityState::Actuator(ActuatorState {
                state: Some(State::Light(_)),
            })) => vec![PayloadTabKind::UpdateFrequency, PayloadTabKind::Light],
            Some(_) => vec![PayloadTabKind::UpdateFrequency],
            None => vec![],
        }
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
                    Some(index) => self.state.keys_stable().nth(index)?,
                    None => self.entity_input.text(),
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

    fn handle_payload_select_event(&self, event: &Event) -> Option<Action> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            }) => Some(Action::SendMessage(match &self.tab {
                PayloadTab::UpdateFrequency(text) => {
                    let freq: f32 = text.text().parse().ok()?;
                    NamedEntityState::frequency(self.entity_input.text(), freq)
                }
                PayloadTab::Light { brightness } => NamedEntityState::actuator(
                    self.entity_input.text(),
                    ActuatorState::light(*brightness),
                ),
                PayloadTab::AirConditioning(list) => {
                    let on = match list.selected()? {
                        0 => false,
                        1 => true,
                        _ => return None,
                    };
                    NamedEntityState::actuator(
                        self.entity_input.text(),
                        ActuatorState::air_conditioning(on),
                    )
                }
            })),
            Event::Key(KeyEvent {
                code: code @ (KeyCode::Tab | KeyCode::BackTab),
                kind: KeyEventKind::Press,
                ..
            }) => {
                let tab_kind: PayloadTabKind = (&*self.tab).into();
                let new_tab = tab_kind.cycle(matches!(code, KeyCode::Tab)).into();
                Some(Action::ChangePayloadTab(new_tab))
            }
            Event::Key(KeyEvent {
                code: KeyCode::Up | KeyCode::Down,
                kind: KeyEventKind::Press,
                ..
            }) if matches!(self.tab, PayloadTab::AirConditioning(..)) => {
                Some(Action::ToggleAirConditioning)
            }
            Event::Key(event) if matches!(self.tab, PayloadTab::UpdateFrequency { .. }) => {
                match event.code {
                    KeyCode::Char(c)
                        if event.modifiers.is_empty() && !c.is_numeric() && c != '.' =>
                    {
                        None
                    }
                    _ => Some(Action::TextInput(Event::Key(*event).into())),
                }
            }
            Event::Key(KeyEvent {
                code: code @ (KeyCode::Left | KeyCode::Right),
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                modifiers,
                ..
            }) => {
                use crossterm::event::KeyModifiers;
                let &mut PayloadTab::Light { brightness } = self.tab else {
                    return None;
                };

                let up = matches!(code, KeyCode::Right);
                let slow = matches!(modifiers, &KeyModifiers::SHIFT);
                let delta = match (up, slow) {
                    (true, true) => 0.1,
                    (false, true) => -0.1,
                    (true, false) => 1.0,
                    (false, false) => -1.0,
                };
                Some(Action::SetLightBrightness(
                    (brightness + delta).clamp(0.0, 100.0),
                ))
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
                SendStage::PayloadSelect {} => self.handle_payload_select_event(&event),
            })
    }
}
