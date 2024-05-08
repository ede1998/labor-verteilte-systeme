use std::{collections::HashMap, time::Duration};

use anyhow::{Context as _, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal,
};
use home_automation_common::{
    protobuf::{
        entity_discovery_command::{Command, EntityType, Registration},
        ActuatorState, EntityDiscoveryCommand, ResponseCode,
    },
    shutdown_requested, zmq_sockets, EntityState, OpenTelemetryConfiguration,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Rect},
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{
        block::{Position, Title},
        Block, Borders,
    },
    Frame, Terminal,
};
use tui_textarea::TextArea;

type Tui = Terminal<CrosstermBackend<std::io::Stdout>>;

/// Initialize the terminal
fn init_raw_tty() -> Result<Tui> {
    terminal::enable_raw_mode().context("Failed to get stdout")?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        &mut stdout,
        terminal::EnterAlternateScreen,
        event::EnableMouseCapture
    )
    .context("Failed to enter alternate screen")?;
    Terminal::new(CrosstermBackend::new(stdout)).context("Failed to create terminal")
}

/// Restore the terminal to its original state
fn restore_normal_tty() -> Result<()> {
    crossterm::execute!(
        std::io::stdout(),
        terminal::LeaveAlternateScreen,
        event::DisableMouseCapture
    )
    .context("Failed to leave alternate screen")?;
    terminal::disable_raw_mode().context("Failed to disable raw_mode")
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

#[derive(Debug, Default, Clone, Copy)]
enum View {
    #[default]
    Monitor,
}

#[derive(Debug)]
pub struct App {
    counter: u8,
    input: TextArea<'static>,
    state: HashMap<String, EntityState>,
    view: View,
}

impl Default for App {
    fn default() -> Self {
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_placeholder_text("Please enter my text here");
        Self {
            counter: Default::default(),
            input,
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
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events().context("handle events failed")?;
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        // use ratatui::layout::{Constraint, Layout};
        // let layout =
        //     Layout::default().constraints([Constraint::Length(3), Constraint::Min(1)].as_slice());
        // let chunks = layout.split(frame.size());

        // frame.render_widget(self.input.widget(), chunks[0]);
        // self.render_message_counter(frame, chunks[1]);
        match self.view {
            View::Monitor => MonitorView(&self.state).render(frame),
        }
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> Result<()> {
        let event = event::read().context("Failed to read input event")?;
        let action = match self.view {
            View::Monitor => MonitorView(&self.state).handle_events(event),
        };
        match action {
            Some(Action::Exit) => home_automation_common::request_shutdown(),
            Some(Action::ChangeView(v)) => self.view = v,
            Some(Action::Refresh) => {}
            Some(Action::ToggleAutoRefresh) => {}
            None => {}
        }
        Ok(())
    }
}

enum Action {
    ChangeView(View),
    Refresh,
    ToggleAutoRefresh,
    Exit,
}

struct MonitorView<'a>(&'a HashMap<String, EntityState>);

impl<'a> MonitorView<'a> {
    fn render_table(&self, frame: &mut Frame, area: Rect) {
        use ratatui::{
            layout::Constraint,
            widgets::{Row, Table},
        };

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
            .rows(self.0.iter().map(|(name, state)| {
                Row::new([
                    name.into(),
                    state.entity_type().to_string().blue(),
                    DisplayEntityState(state).to_string().into(),
                ])
            }));

        frame.render_widget(table, area);
    }

    fn render(&self, frame: &mut Frame) {
        let instructions = Title::from(Line::from(vec![
            " Send Message ".into(),
            "<S>".blue().bold(),
            " Refresh ".into(),
            "<R>".blue().bold(),
            " Auto-Refresh ".into(),
            "<CTRL-R>".blue().bold(),
            " Quit ".into(),
            "<Q> ".blue().bold(),
        ]));
        let block = prepare_scaffolding(instructions);

        frame.render_widget(&block, frame.size());
        self.render_table(frame, block.inner(frame.size()));
    }

    fn handle_events(&self, event: event::Event) -> Option<Action> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('s'),
                ..
            }) => Some(Action::ChangeView(todo!())),
            Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                ..
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
fn main() -> Result<()> {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_normal_tty().unwrap();
        default_hook(info);
    }));

    let result = init_raw_tty().and_then(|mut tui| {
        let mut app = App::default();
        app.run(&mut tui)
    });
    restore_normal_tty()?;
    return result;

    let _config = OpenTelemetryConfiguration::new("client")?;
    tracing::info_span!("main").in_scope(|| {
        tracing::info!("Starting controller");
        let context = zmq_sockets::Context::new();
        let client = zmq_sockets::Requester::new(&context)?.connect("tcp://localhost:5556")?;
        // TODO: implement client properly

        while !shutdown_requested() {
            let _ = send_entity(&context, &client);
            std::thread::sleep(Duration::from_millis(1000));
        }
        Ok(())
    })
}

#[tracing::instrument(parent=None, skip_all, err)]
fn send_entity(
    context: &zmq_sockets::Context,
    client: &zmq_sockets::Requester<zmq_sockets::markers::Linked>,
) -> Result<()> {
    let rep = zmq_sockets::Replier::new(context)?.bind("tcp://*:*")?;
    let ep = rep.get_last_endpoint()?;
    let request = EntityDiscoveryCommand {
        command: Command::Register(Registration {
            port: ep.port().into(),
        })
        .into(),
        entity_name: "asd".to_owned(),
        entity_type: EntityType::Sensor.into(),
    };

    tracing::debug!("Sending {request:?}");
    client.send(request)?;

    let response_code: ResponseCode = client.receive()?;
    tracing::debug!("Received {response_code:?}");

    let response_code: ResponseCode = rep.receive()?;
    tracing::debug!("HALLELUJAH {response_code:?}");

    Ok(())
}
