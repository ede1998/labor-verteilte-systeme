use std::sync::mpsc::Sender;

use anyhow::Context as _;
use home_automation_common::{
    load_env,
    protobuf::{entity_discovery_command, EntityDiscoveryCommand, ResponseCode},
    shutdown_requested,
    zmq_sockets::{self, markers::Linked, termination_is_ok},
};

use crate::state::{AppState, Entity, SubscriptionCommand};

pub struct EntityDiscoveryTask<'a> {
    app_state: &'a AppState,
    server: zmq_sockets::Replier<Linked>,
    new_subscriptions: Sender<SubscriptionCommand>,
}

impl<'a> EntityDiscoveryTask<'a> {
    pub fn new(
        app_state: &'a AppState,
        new_subscriptions: Sender<SubscriptionCommand>,
    ) -> anyhow::Result<Self> {
        let address = load_env(home_automation_common::ENV_DISCOVERY_ENDPOINT)?;
        let server = zmq_sockets::Replier::new(&app_state.context)?.bind(&address)?;
        Ok(Self {
            app_state,
            server,
            new_subscriptions,
        })
    }

    #[tracing::instrument(name = "entity discovery", skip(self))]
    pub fn run(&self) -> anyhow::Result<()> {
        while !shutdown_requested() {
            let Err(e) = self.accept_entity() else {
                continue;
            };
            return Err(e)
                .or_else(termination_is_ok)
                .inspect_err(|e| tracing::error!(%e, "Failed to to handle entity request: {e:#}"));
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn accept_entity(&self) -> anyhow::Result<()> {
        let (request, ip): (EntityDiscoveryCommand, _) = self.server.receive_with_ip()?;

        let result = self.handle_command(request, ip);
        tracing::info!(?result, "Finished handling command with result {result:?}");

        let response: ResponseCode = result.into();
        self.server.send(response)?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn handle_command(&self, request: EntityDiscoveryCommand, ip: String) -> anyhow::Result<()> {
        use dashmap::mapref::entry::Entry;
        use entity_discovery_command::Command;
        let entity_type = request.entity_type();
        match request.command {
            Some(Command::Register(registration)) => {
                match self.app_state.entities.entry(request.entity_name.clone()) {
                    Entry::Occupied(o) => {
                        anyhow::bail!("Entity {} already registered", o.key());
                    }
                    Entry::Vacant(v) => {
                        tracing::info!("Registering entity {}", v.key());
                        let requester = self
                            .open_back_channel(ip, registration.port)
                            .context("Failed to create back-channel")?;
                        self.new_subscriptions.send(SubscriptionCommand::subscribe(
                            home_automation_common::entity_topic(&request.entity_name, entity_type),
                        ))?;
                        v.insert(Entity::new(requester));
                    }
                }
            }
            Some(Command::Unregister(())) => {
                self.new_subscriptions
                    .send(SubscriptionCommand::unsubscribe(
                        home_automation_common::entity_topic(&request.entity_name, entity_type),
                    ))?;
                self.app_state.unregister(&request.entity_name)?;
            }
            Some(Command::Heartbeat(())) => {
                let mut entity = self
                    .app_state
                    .entities
                    .get_mut(&request.entity_name)
                    .with_context(|| {
                        anyhow::anyhow!("Heartbeat from unknown entity {}", request.entity_name)
                    })?;
                entity.last_heartbeat_pulse = std::time::Instant::now();
            }
            None => anyhow::bail!("EntityDiscoveryCommand is missing the command"),
        }
        Ok(())
    }

    fn open_back_channel(
        &self,
        ip: String,
        port: u32,
    ) -> anyhow::Result<zmq_sockets::Requester<Linked>> {
        zmq_sockets::Requester::new(&self.app_state.context)
            .context("Failed to create back-channel socket")?
            .connect(&format!("tcp://{ip}:{port}"))
            .context("Failed to connect back-channel socket")
    }
}
