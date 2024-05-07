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
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{
        block::{Position, Title},
        Block, Borders, Paragraph, Widget,
    },
    Frame, Terminal,
};

type Tui = Terminal<CrosstermBackend<std::io::Stdout>>;

/// Initialize the terminal
fn init_raw_tty() -> Result<Tui> {
    terminal::enable_raw_mode().context("Failed to get stdout")?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(&mut stdout, terminal::EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    Terminal::new(CrosstermBackend::new(stdout)).context("Failed to create terminal")
}

/// Restore the terminal to its original state
fn restore_normal_tty() -> Result<()> {
    crossterm::execute!(std::io::stdout(), terminal::LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    terminal::disable_raw_mode().context("Failed to disable raw_mode")
}

#[derive(Debug, Default)]
pub struct App {
    counter: u8,
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
        frame.render_widget(self, frame.size());
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => self
                .handle_key_event(key_event)
                .with_context(|| anyhow::anyhow!("handling key event failed:\n{key_event:#?}")),
            _ => Ok(()),
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Char('q') => {
                home_automation_common::request_shutdown();
            }
            KeyCode::Left => self.decrement_counter()?,
            KeyCode::Right => self.increment_counter()?,
            _ => {}
        }
        Ok(())
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

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
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

        Paragraph::new(counter_text)
            .centered()
            .block(block)
            .render(area, buf);
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
