use std::collections::HashMap;

use anyhow::{Context as _, Result};
use crossterm::event;
use home_automation_common::{
    protobuf::{entity_discovery_command::EntityType, ActuatorState},
    EntityState,
};

use super::{
    view::{SendStage, UiView, View},
    Tui,
};

pub enum Action {
    ChangeView(View),
    Refresh,
    ToggleAutoRefresh,
    Exit,
    SetMessageRecipient(String),
    SetRecipientSelection(Option<usize>),
    TextInput(tui_textarea::Input),
}

#[derive(Debug)]
pub struct App {
    state: HashMap<String, EntityState>,
    view: View,
}

impl Default for App {
    fn default() -> Self {
        Self {
            view: View::default(),
            state: HashMap::from([
                ("Peter".to_owned(), EntityState::New(EntityType::Sensor)),
                (
                    "Frank".to_owned(),
                    EntityState::Actuator(ActuatorState::light(77.2)),
                ),
            ]),
        }
    }
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while !home_automation_common::shutdown_requested() {
            terminal.draw(|frame| self.view.active(&self.state).render(frame))?;
            self.handle_events().context("Failed to handle events")?;
        }
        Ok(())
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> Result<()> {
        use ratatui::style::Modifier;
        let event = event::read().context("Failed to read input event")?;
        let action = self.view.active(&self.state).handle_events(event);
        match action {
            Some(Action::Exit) => home_automation_common::request_shutdown(),
            Some(Action::ChangeView(v)) => self.view = v,
            Some(Action::Refresh) => todo!(),
            Some(Action::ToggleAutoRefresh) => todo!(),
            Some(Action::SetMessageRecipient(recipient)) => {
                let (input, _, stage) = self.view.ensure_send_mut();
                input.cancel_selection();
                input.select_all();
                input.insert_str(recipient);
                input.set_cursor_style(Default::default());
                *stage = SendStage::PayloadSelect {};
            }
            Some(Action::SetRecipientSelection(index)) => {
                let (input, list, _) = self.view.ensure_send_mut();
                list.select(index);
                input.set_cursor_style(if index.is_some() {
                    Default::default()
                } else {
                    Modifier::REVERSED.into()
                });
            }
            Some(Action::TextInput(keyboard_input)) => {
                let (input, _, _) = self.view.ensure_send_mut();
                input.input(keyboard_input);
                input.set_cursor_style(Modifier::REVERSED.into());
            }
            None => {}
        }
        Ok(())
    }
}
