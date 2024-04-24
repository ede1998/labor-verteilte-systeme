use anyhow::{Context as _, Result};
use home_automation_common::{
    load_env,
    protobuf::{
        entity_discovery_command::{Command, EntityType, Registration},
        response_code::Code,
        EntityDiscoveryCommand, ResponseCode,
    },
    zmq_sockets::{self, markers::Linked},
};

struct App {
    context: zmq_sockets::Context,
    data_endpoint: String,
    discovery_endpoint: String,
    name: String,
}

impl App {
    pub fn new() -> Result<Self> {
        let actuator_name = std::env::args()
            .skip(1)
            .next()
            .context("Missing actuator name.")?;
        Ok(Self {
            context: zmq_sockets::Context::new(),
            data_endpoint: load_env(home_automation_common::ENV_ENTITY_DATA_ENDPOINT)?,
            discovery_endpoint: load_env(home_automation_common::ENV_DISCOVERY_ENDPOINT)?,
            name: format!("act_{actuator_name}"),
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

    /// Publishes a single sample.
    #[tracing::instrument(parent=None, skip_all)]
    fn publish_data(&self, publisher: &zmq_sockets::Publisher<Linked>) -> Result<()> {
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let app = App::new()?;
    let _config = home_automation_common::OpenTelemetryConfiguration::new(&app.name)?;
    home_automation_common::install_signal_handler(app.context.clone())?;

    let update_replier = zmq_sockets::Replier::new(&app.context)?.bind("tcp://*:*")?;
    let update_endpoint = update_replier.get_last_endpoint()?;

    let heartbeat_socket = app.connect(update_endpoint.port())?;
    // spawn PUB socket
    // spawn REQ socket
    // spawn REP socket
    // connect to server
    // start tasks
    // - heartbeat
    // - pub
    // - settings update
    std::thread::scope(|s| {
        let discovery = s.spawn(|| EntityDiscoveryTask::new(&app_estate)?.run());
        let client_api = s.spawn(|| ClientApiTask::new(&app_state)?.run());

        discovery
            .join()
            .map_err(|e| anyhow::anyhow!("Entity discovery task panicked: {e:?}"))?
            .context("Entity discovery task failed")?;
        client_api
            .join()
            .map_err(|e| anyhow::anyhow!("Client API task panicked: {e:?}"))?
            .context("Client API task failed")?;
        Ok(())
    })
}
