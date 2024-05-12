use std::collections::HashMap;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use home_automation_common::EntityState;
use ratatui::{
    layout::{Constraint, Rect},
    style::Stylize as _,
    text::Line,
    widgets::block::Title,
    Frame,
};

use crate::{ui::app::Action, utility::HashMapExt};

use super::{prepare_scaffolding, UiView, View};

pub struct MonitorView<'a>(pub &'a HashMap<String, EntityState>);

impl<'a> MonitorView<'a> {
    fn render_table(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Row, Table};

        struct DisplayEntityState<'a>(&'a EntityState);

        impl<'a> std::fmt::Display for DisplayEntityState<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                use home_automation_common::protobuf::{
                    actuator_state::State, sensor_measurement::Value, ActuatorState,
                    SensorMeasurement,
                };
                match self.0 {
                    EntityState::Sensor(SensorMeasurement {
                        unit,
                        value: Some(Value::Humidity(h)),
                    }) => write!(f, "humidity = {}{unit}", h.humidity),
                    EntityState::Sensor(SensorMeasurement {
                        unit,
                        value: Some(Value::Temperature(t)),
                    }) => write!(f, "temperature = {}{unit}", t.temperature),
                    EntityState::Actuator(ActuatorState {
                        state: Some(State::Light(l)),
                    }) => write!(f, "brightness = {}%", l.brightness),
                    EntityState::Actuator(ActuatorState {
                        state: Some(State::AirConditioning(ac)),
                    }) => write!(f, "on = {}", ac.on),
                    _ => Ok(()),
                }
            }
        }

        let table = Table::default()
            .header(
                Row::new(["Entity", "Type", "Value"])
                    .bold()
                    .underlined()
                    .blue(),
            )
            .widths([
                Constraint::Min(20),
                Constraint::Length(8),
                Constraint::Percentage(80),
            ])
            .rows(self.0.iter_stable().map(|(name, state)| {
                Row::new([
                    name.into(),
                    state.entity_type().to_string().blue(),
                    DisplayEntityState(state).to_string().into(),
                ])
            }));

        frame.render_widget(table, area);
    }
}

impl<'a> UiView for MonitorView<'a> {
    fn render(&mut self, frame: &mut Frame) {
        let instructions = Title::from(Line::from(vec![
            " Send Message ".into(),
            "<S>".blue().bold(),
            " Refresh ".into(),
            "<R>".blue().bold(),
            " Auto-Refresh ".into(),
            "<CTRL-R>".blue().bold(),
            " Quit ".into(),
            "<ESC> ".blue().bold(),
        ]));
        let block = prepare_scaffolding(instructions);

        frame.render_widget(&block, frame.size());
        self.render_table(frame, block.inner(frame.size()));
    }

    fn handle_events(&self, event: Event) -> Option<Action> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('s'),
                kind: KeyEventKind::Press,
                ..
            }) => Some(Action::ChangeView(View::Send(Default::default()))),
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) => Some(Action::Exit),
            Event::Key(KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) => Some(Action::Refresh),
            Event::Key(KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }) => Some(Action::ToggleAutoRefresh),
            _ => None,
        }
    }
}
