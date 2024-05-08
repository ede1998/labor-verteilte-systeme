use std::time::Duration;

use anyhow::{Context as _, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    terminal,
};
use home_automation_common::{
    protobuf::{
        entity_discovery_command::{Command, EntityType, Registration},
        EntityDiscoveryCommand, ResponseCode,
    },
    shutdown_requested, zmq_sockets, OpenTelemetryConfiguration,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{
        block::{Position, Title},
        Block, Borders, Paragraph,
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

struct Item {
    user: &'static str,
    mail: &'static str,
    timezone: &'static str,
}

const ITEMS: [Item; 6] = [
    Item {
        user: "fdehau",
        mail: "Florian Dehau <fdehau@users.noreply.github.com",
        timezone: "UTC+1",
    },
    Item {
        user: "joshka",
        mail: "Josh McKinney <joshka@users.noreply.github.com>",
        timezone: "UTC-8",
    },
    Item {
        user: "kdheepak",
        mail: "Dheepak Krishnamurthy <me@kdheepak.com>",
        timezone: "UTC-5",
    },
    Item {
        user: "mindoodoo",
        mail: "Leon Sautour <minindoo@users.noreply.github.com>",
        timezone: "UTC+1",
    },
    Item {
        user: "orhun",
        mail: "Orhun Parmaksız <orhun@archlinux.org>",
        timezone: "UTC+3",
    },
    Item {
        user: "Valentin271",
        mail: "Valentin271 <36198422+Valentin271@users.noreply.github.com>",
        timezone: "UTC+1",
    },
];
#[derive(Debug)]
pub struct App {
    counter: u8,
    input: TextArea<'static>,
}

impl Default for App {
    fn default() -> Self {
        let mut input = TextArea::default();
        input.set_cursor_line_style(ratatui::style::Style::default());
        input.set_placeholder_text("Please enter my text here");
        Self {
            counter: Default::default(),
            input,
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

    fn render_table(&self, frame: &mut Frame, area: Rect) {
        use ratatui::{
            layout::Constraint,
            widgets::{Row, Table},
        };
        let rows = ITEMS.iter().map(|item| {
            Row::new([
                item.user.into(),
                item.mail.dim().into(),
                Line::from(item.timezone).alignment(Alignment::Right),
            ])
        });
        // width of 46
        let table = Table::new(
            rows,
            [
                Constraint::Length(11),
                Constraint::Length(29),
                Constraint::Length(5),
            ],
        )
        .header(Row::new(["User", "Email", "TZ"]).bold().underlined().blue());
        frame.render_widget(table, area);
    }

    fn render_message_counter(&self, frame: &mut Frame, area: Rect) {
        let title = Title::from(" Counter App Tutorial ".bold());
        let instructions = Title::from(Line::from(vec![
            " Decrement ".into(),
            "<Left>".blue().bold(),
            " Increment ".into(),
            "<Right>".blue().bold(),
            " Quit ".into(),
            "<Q> ".blue().bold(),
        ]));
        let block = Block::default()
            .title(title.alignment(Alignment::Center))
            .title(
                instructions
                    .alignment(Alignment::Center)
                    .position(Position::Bottom),
            )
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let counter_text = Text::from(vec![Line::from(vec![
            "Value: ".into(),
            self.counter.to_string().yellow(),
        ])]);

        frame.render_widget(Paragraph::new(counter_text).centered().block(block), area);
    }

    fn render_frame(&self, frame: &mut Frame) {
        // use ratatui::layout::{Constraint, Layout};
        // let layout =
        //     Layout::default().constraints([Constraint::Length(3), Constraint::Min(1)].as_slice());
        // let chunks = layout.split(frame.size());

        // frame.render_widget(self.input.widget(), chunks[0]);
        // self.render_message_counter(frame, chunks[1]);
        self.render_table(frame, frame.size());
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> Result<()> {
        match event::read()? {
            Event::Key(KeyEvent {
                code: code @ (KeyCode::Char('q') | KeyCode::Left | KeyCode::Right),
                kind,
                ..
            }) => {
                if kind != KeyEventKind::Press {
                    return Ok(());
                }
                match code {
                    KeyCode::Char('q') => {
                        home_automation_common::request_shutdown();
                    }
                    KeyCode::Left => self
                        .decrement_counter()
                        .context("Failed to decrement counter")?,
                    KeyCode::Right => self
                        .increment_counter()
                        .context("Failed to increment counter")?,
                    _ => {}
                }
                Ok(())
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                ..
            }) => Ok(()),
            event => {
                self.input.input(event);
                Ok(())
            }
        }
    }

    fn decrement_counter(&mut self) -> Result<()> {
        self.counter -= 1;
        Ok(())
    }

    fn increment_counter(&mut self) -> Result<()> {
        self.counter += 1;
        anyhow::ensure!(self.counter <= 2, "counter overflow");
        Ok(())
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
