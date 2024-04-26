use std::{sync::RwLock, time::Duration};

use anyhow::{Context as _, Result};
use home_automation_common::{
    actuator_state_topic, load_env,
    protobuf::{
        entity_discovery_command::{Command, EntityType, Registration},
        response_code::Code,
        ActuatorState, EntityDiscoveryCommand, ResponseCode,
    },
    zmq_sockets::{self, markers::Linked},
    HEARTBEAT_FREQUENCY,
};

struct App {
    context: zmq_sockets::Context,
    data_endpoint: String,
    discovery_endpoint: String,
    name: String,
    data: RwLock<ActuatorState>,
    refresh_rate: Duration,
}

impl App {
    pub fn new() -> Result<Self> {
        let actuator_name = std::env::args().nth(1).context("Missing actuator name.")?;
        Ok(Self {
            context: zmq_sockets::Context::new(),
            data_endpoint: load_env(home_automation_common::ENV_ENTITY_DATA_ENDPOINT)?,
            discovery_endpoint: load_env(home_automation_common::ENV_DISCOVERY_ENDPOINT)?,
            name: format!("act_{actuator_name}"),
            data: RwLock::new(ActuatorState { state: None }),
            refresh_rate: Duration::from_millis(1500),
        })
    }

    fn discovery_command(&self, command: Command) -> EntityDiscoveryCommand {
        EntityDiscoveryCommand {
            command: Some(command),
            entity_name: self.name.clone(),
            entity_type: EntityType::Actuator.into(),
        }
    }

    #[tracing::instrument(skip(self))]
    fn connect(&self, update_port: u16) -> Result<zmq_sockets::Requester<Linked>> {
        let requester =
            zmq_sockets::Requester::new(&self.context)?.connect(&self.discovery_endpoint)?;

        let request = self.discovery_command(Command::Register(Registration {
            port: update_port.into(),
        }));

        tracing::info!("Sending connect request {request:?}");
        requester.send(request)?;

        let response_code: ResponseCode = requester.receive()?;
        tracing::debug!("Received {response_code:?}");

        Ok(requester)
    }

    fn run_heartbeat(&self, requester: &zmq_sockets::Requester<Linked>) -> Result<()> {
        loop {
            std::thread::sleep(HEARTBEAT_FREQUENCY);
            self.heartbeat(requester)
                .inspect_err(|_| home_automation_common::request_shutdown())?;
        }
    }

    /// Sends a single heartbeat and waits for the answer.
    #[tracing::instrument(parent=None, skip_all)]
    fn heartbeat(&self, requester: &zmq_sockets::Requester<Linked>) -> Result<()> {
        let request = self.discovery_command(Command::Heartbeat(()));
        tracing::info!("Sending heartbeat request {request:?}");
        requester.send(request)?;
        let response: ResponseCode = requester.receive()?;
        match response.code() {
            Code::Ok => Ok(()),
            Code::Error => anyhow::bail!("Heartbeat failed"),
        }
    }

    fn run_publish_data(&self, publisher: zmq_sockets::Publisher<Linked>) -> Result<()> {
        let mut error_counter = 0;
        loop {
            match self.publish_data(&publisher) {
                Err(e) if error_counter > 3 => return Err(e),
                Err(e) => {
                    tracing::error!(error=%e, "Failed to publish data: {e:#}");
                    error_counter += 1;
                }
                Ok(_) => {
                    error_counter = 0;
                }
            }
            std::thread::sleep(self.refresh_rate);
        }
    }

    /// Publishes a single sample.
    #[tracing::instrument(parent=None, skip_all)]
    fn publish_data(&self, publisher: &zmq_sockets::Publisher<Linked>) -> Result<()> {
        let data = self.data.read().expect("non-poisoned RwLock").clone();
        publisher
            .send(actuator_state_topic(&self.name), data)
            .context("Failed to publish data")
    }

    fn run_updater(&self, updater: zmq_sockets::Replier<Linked>) -> Result<()> {
        todo!()
    }

    #[tracing::instrument(parent=None, skip_all)]
    fn update(&self, updater: &zmq_sockets::Replier<Linked>) -> Result<()> {
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let app = App::new()?;
    let _config = home_automation_common::OpenTelemetryConfiguration::new(&app.name)?;
    home_automation_common::install_signal_handler(app.context.clone())?;

    let update_replier = zmq_sockets::Replier::new(&app.context)?.bind("tcp://*:*")?;
    let update_endpoint = update_replier.get_last_endpoint()?;
    let publisher = zmq_sockets::Publisher::new(&app.context)?.connect(&app.data_endpoint)?;

    let heartbeat_socket = app.connect(update_endpoint.port())?;
    std::thread::scope(|s| {
        let publisher = s.spawn({
            let app = &app;
            move || app.run_publish_data(publisher)
        });
        let updater = s.spawn({
            let app = &app;
            move || app.run_updater(update_replier)
        });

        app.run_heartbeat(&heartbeat_socket)?;
        publisher
            .join()
            .map_err(|e| anyhow::anyhow!("Publisher task panicked: {e:?}"))?
            .context("Publisher task failed")?;
        updater
            .join()
            .map_err(|e| anyhow::anyhow!("Updater task panicked: {e:?}"))?
            .context("Updater task failed")?;
        Ok(())
    })
}
