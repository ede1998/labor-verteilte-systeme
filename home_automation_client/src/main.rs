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
    layout::{Alignment, Constraint, Layout, Rect},
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{
        block::{Position, Title},
        Block, Borders, ListState,
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

trait UiView {
    fn handle_events(&self, event: event::Event) -> Option<Action>;
    fn render(&mut self, frame: &mut Frame);
}

#[derive(Debug, Default, Clone)]
enum View {
    #[default]
    Monitor,
    Send {
        input: TextArea<'static>,
        list: ListState,
    },
}

impl View {
    fn send() -> Self {
        let list = ListState::default();
        let mut input = TextArea::default();
        input.set_cursor_line_style(Default::default());
        input.set_block(Block::bordered());
        Self::Send { input, list }
    }

    fn active<'a>(&'a mut self, state: &'a HashMap<String, EntityState>) -> impl UiView + 'a {
        macro_rules! all_views {
            ($($view:ident),+) => {
                enum Views<'b> {
                    $($view($view<'b>),)+
                }
                impl<'b> UiView for Views<'b> {
                    fn handle_events(&self, event: event::Event) -> Option<Action> {
                        match self {
                            $(Self::$view(v) => v.handle_events(event),)+
                        }
                    }

                    fn render(&mut self, frame: &mut Frame) {
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
            Self::Send { input, list } => Views::SendView(SendView {
                state,
                entity_input: input,
                list,
            }),
        }
    }
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
            .rows(self.0.iter().map(|(name, state)| {
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

    fn handle_events(&self, event: event::Event) -> Option<Action> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('s'),
                kind: KeyEventKind::Press,
                ..
            }) => Some(Action::ChangeView(View::send())),
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

struct SendView<'a> {
    state: &'a HashMap<String, EntityState>,
    entity_input: &'a TextArea<'a>,
    list: &'a mut ListState,
}

impl<'a> SendView<'a> {
    fn render_name_select(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::{
            style::{Color, Modifier},
            text::Span,
            widgets::{BorderType, List},
        };

        let container = Block::bordered()
            .title("Entity".bold().blue())
            .border_type(BorderType::Thick)
            .border_style(Color::Blue);
        frame.render_widget(&container, area);

        let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(5)]);
        let [input_area, list_area] = *layout.split(container.inner(area)) else {
            panic!("Failed to setup layout");
        };

        frame.render_widget(self.entity_input.widget(), input_area);

        let list = List::new(self.state.keys().map(Span::raw))
            .block(Block::default().borders(Borders::ALL))
            // invert color scheme for selected line
            .highlight_style(Modifier::REVERSED);

        frame.render_stateful_widget(list, list_area, self.list);
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
    }

    fn handle_events(&self, event: event::Event) -> Option<Action> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            }) => todo!(),
            Event::Key(KeyEvent {
                code: KeyCode::Tab,
                kind: KeyEventKind::Press,
                ..
            }) => todo!(),
            Event::Key(KeyEvent {
                code: KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right,
                kind: KeyEventKind::Press,
                ..
            }) => todo!(),
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) => Some(Action::ChangeView(View::Monitor)),
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
