use std::collections::HashMap;

use anyhow::{Context as _, Result};
use crossterm::event;
use home_automation_common::{
    protobuf::{entity_discovery_command::EntityType, ActuatorState, NamedEntityState},
    EntityState,
};

use super::{
    view::{PayloadTab, SendStage, UiView, View},
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
    SendMessage(NamedEntityState),
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
        let event = event::read().context("Failed to read input event")?;
        let action = self.view.active(&self.state).handle_events(event);
        match action {
            Some(Action::Exit) => home_automation_common::request_shutdown(),
            Some(Action::ChangeView(v)) => self.view = v,
            Some(Action::Refresh) => todo!(),
            Some(Action::ToggleAutoRefresh) => todo!(),
            Some(Action::SetMessageRecipient(recipient)) => {
                let send_data = self.view.ensure_send_mut();
                send_data.input.cancel_selection();
                send_data.input.select_all();
                send_data.input.insert_str(recipient);
                send_data.list.select(None);
                send_data.stage = SendStage::PayloadSelect {};
            }
            Some(Action::SetRecipientSelection(index)) => {
                let send_data = self.view.ensure_send_mut();
                send_data.list.select(index);
            }
            Some(Action::TextInput(input)) => {
                let send_data = self.view.ensure_send_mut();
                send_data.list.select(None);
                if matches!(send_data.stage, SendStage::EntitySelect) {
                    send_data.input.input(input);
                } else if let PayloadTab::UpdateFrequency(freq_input) = &mut send_data.tab {
                    freq_input.input(input);
                }
            }
            Some(Action::SendMessage(_)) => todo!(),
            None => {}
        }
        Ok(())
    }
}
