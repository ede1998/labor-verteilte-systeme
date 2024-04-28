use std::{sync::RwLock, time::Duration};

use anyhow::{Context as _, Result};
use home_automation_common::{
    load_env,
    protobuf::{
        entity_discovery_command::{Command, EntityType, Registration},
        response_code::Code,
        EntityDiscoveryCommand, NamedEntityState, ResponseCode,
    },
    zmq_sockets::{self, markers::Linked},
    HEARTBEAT_FREQUENCY,
};

pub trait Entity: Sync {
    const ENTITY_TYPE: EntityType;

    fn new(base_name: String) -> Result<Self>
    where
        Self: Sized;
    fn name(&self) -> &str;
    fn topic_name(&self) -> &str;

    type PublishData: Send + Sync + Clone + prost::Message + prost::Name + Default + std::fmt::Debug;
    fn retrieve_publish_data(&self) -> Self::PublishData;
    fn handle_incoming_data(&self, data: NamedEntityState) -> Result<Option<Duration>>;
}

pub struct Sockets {
    pub publisher: zmq_sockets::Publisher<Linked>,
    pub replier: zmq_sockets::Replier<Linked>,
    pub heartbeat: zmq_sockets::Requester<Linked>,
}

pub struct App<E: Entity> {
    context: zmq_sockets::Context,
    data_endpoint: String,
    discovery_endpoint: String,
    pub entity: E,
    pub refresh_rate: RwLock<Duration>,
}

impl<E: Entity> App<E> {
    pub fn new() -> Result<Self> {
        let name = std::env::args().nth(1).context("Missing name.")?;
        let context = zmq_sockets::Context::new();
        home_automation_common::install_signal_handler(context.clone())?;
        Ok(Self {
            context,
            data_endpoint: load_env(home_automation_common::ENV_ENTITY_DATA_ENDPOINT)?,
            discovery_endpoint: load_env(home_automation_common::ENV_DISCOVERY_ENDPOINT)?,
            entity: E::new(name).context("Failed to create entity")?,
            refresh_rate: RwLock::new(Duration::from_millis(1500)),
        })
    }

    pub fn run(&self, sockets: Sockets) -> Result<()> {
        std::thread::scope(|s| {
            let publisher = s.spawn(move || self.run_publish_data(sockets.publisher));
            let updater = s.spawn(move || self.run_updater(sockets.replier));

            self.run_heartbeat(sockets.heartbeat)?;
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

    // TODO: disconnect request on stop

    fn discovery_command(&self, command: Command) -> EntityDiscoveryCommand {
        EntityDiscoveryCommand {
            command: Some(command),
            entity_name: self.entity.name().to_owned(),
            entity_type: E::ENTITY_TYPE.into(),
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn connect(&self) -> Result<Sockets> {
        let replier = zmq_sockets::Replier::new(&self.context)?.bind("tcp://*:*")?;
        let update_port = replier.get_last_endpoint()?.port();
        let publisher = zmq_sockets::Publisher::new(&self.context)?.connect(&self.data_endpoint)?;

        let requester =
            zmq_sockets::Requester::new(&self.context)?.connect(&self.discovery_endpoint)?;

        let request = self.discovery_command(Command::Register(Registration {
            port: update_port.into(),
        }));

        tracing::info!("Sending connect request {request:?}");
        requester.send(request)?;

        let response_code: ResponseCode = requester.receive()?;
        tracing::debug!("Received {response_code:?}");

        Ok(Sockets {
            publisher,
            replier,
            heartbeat: requester,
        })
    }

    pub fn run_heartbeat(&self, requester: zmq_sockets::Requester<Linked>) -> Result<()> {
        loop {
            std::thread::sleep(HEARTBEAT_FREQUENCY);
            self.heartbeat(&requester)
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

    pub fn run_publish_data(&self, publisher: zmq_sockets::Publisher<Linked>) -> Result<()> {
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
            std::thread::sleep(*self.refresh_rate.read().expect("non-poisoned RwLock"));
        }
    }

    /// Publishes a single sample.
    #[tracing::instrument(parent=None, skip_all)]
    fn publish_data(&self, publisher: &zmq_sockets::Publisher<Linked>) -> Result<()> {
        let data = self.entity.retrieve_publish_data();
        publisher
            .send(self.entity.topic_name(), data)
            .context("Failed to publish data")
    }

    fn run_updater(&self, updater: zmq_sockets::Replier<Linked>) -> Result<()> {
        while !home_automation_common::shutdown_requested() {
            self.update(&updater)?;
        }
        Ok(())
    }

    /// Read an incoming configuration update and apply it to the entity.
    #[tracing::instrument(parent=None, skip_all)]
    fn update(&self, updater: &zmq_sockets::Replier<Linked>) -> Result<()> {
        let data: NamedEntityState = updater
            .receive()
            .context("Failed to receive config update")?;

        let result = self.entity.handle_incoming_data(data);

        match &result {
            Err(e) => tracing::error!(error=%e, "Failed to apply configuration update: {e:#}"),
            Ok(None) => {
                tracing::info!("Successfully applied configuration update without new refresh rate")
            }
            &Ok(Some(new_refresh_rate)) => {
                *self.refresh_rate.write().expect("non-poisoned RwLock") = new_refresh_rate;
                tracing::info!("Successfully applied configuration update with new refresh rate {new_refresh_rate:?}");
            }
        }

        let code: ResponseCode = result.into();
        updater.send(code)
    }
}
