use std::{collections::HashMap, time::Duration};

use anyhow::{Context as _, Result};
use crossterm::event;
use home_automation_common::{protobuf::NamedEntityState, EntityState};

use crate::network::SystemStateRefresher;

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
    ChangePayloadTab(PayloadTab),
    ToggleAirConditioning,
    SetLightBrightness(f32),
}

#[derive(Debug)]
pub struct BackgroundTaskState<'a> {
    pub refresher: &'a SystemStateRefresher,
    pub receiver: std::sync::mpsc::Receiver<HashMap<String, EntityState>>,
}

#[derive(Debug)]
pub struct App<'a> {
    state: HashMap<String, EntityState>,
    view: View,
    background_task_state: BackgroundTaskState<'a>,
}

impl<'a> App<'a> {
    pub fn new(background_task_state: BackgroundTaskState<'a>) -> Self {
        Self {
            view: View::default(),
            state: HashMap::default(),
            background_task_state,
        }
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        while !home_automation_common::shutdown_requested() {
            terminal.draw(|frame| self.view.active(&self.state).render(frame))?;
            self.handle_events().context("Failed to handle events")?;
            if let Some(new_state) = self.background_task_state.receiver.try_iter().last() {
                self.state = new_state;
            }
        }
        Ok(())
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> Result<()> {
        let event = {
            let context = "Failed to read input event";
            if !event::poll(Duration::from_millis(500)).context(context)? {
                return Ok(());
            }
            event::read().context(context)?
        };
        let action = self.view.active(&self.state).handle_events(event);
        match action {
            Some(Action::Exit) => home_automation_common::request_shutdown(),
            Some(Action::ChangeView(v)) => self.view = v,
            Some(Action::Refresh) => self.background_task_state.refresher.refresh(),
            Some(Action::ToggleAutoRefresh) => {
                self.background_task_state.refresher.toggle_auto_refresh();
            }
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
            Some(Action::ChangePayloadTab(tab)) => {
                let send_data = self.view.ensure_send_mut();
                send_data.tab = tab;
            }
            Some(Action::ToggleAirConditioning) => {
                use crate::utility::Wrapping;
                let send_data = self.view.ensure_send_mut();
                if let PayloadTab::AirConditioning(list) = &mut send_data.tab {
                    let current = Wrapping::new(list.selected().unwrap_or_default(), 1);
                    list.select(Some(current.inc().current()));
                }
            }
            Some(Action::SetLightBrightness(desired_brightness)) => {
                let send_data = self.view.ensure_send_mut();
                if let PayloadTab::Light { brightness } = &mut send_data.tab {
                    *brightness = desired_brightness;
                }
            }
            None => {}
        }
        Ok(())
    }
}
