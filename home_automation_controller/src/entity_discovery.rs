use anyhow::Context as _;
use home_automation_common::{
    protobuf::{entity_discovery_command, response_code, EntityDiscoveryCommand, ResponseCode},
    shutdown_requested,
    zmq_sockets::{self, markers::Linked},
};

use crate::state::{AppState, Entity};

pub struct EntityDiscoveryTask<'a> {
    app_state: &'a AppState,
    server: zmq_sockets::Replier<Linked>,
}

impl<'a> EntityDiscoveryTask<'a> {
    pub fn new(app_state: &'a AppState) -> anyhow::Result<Self> {
        let address = "tcp://*:5556";
        let server = zmq_sockets::Replier::new(&app_state.context)?.bind(address)?;
        Ok(Self { app_state, server })
    }

    #[tracing::instrument(skip(self))]
    pub fn run(&self) -> anyhow::Result<()> {
        while !shutdown_requested() {
            let _ = self.accept_entity();
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    fn accept_entity(&self) -> anyhow::Result<()> {
        use dashmap::mapref::entry::Entry;
        use entity_discovery_command::Command;
        use response_code::Code;
        let (request, ip): (EntityDiscoveryCommand, _) = self.server.receive_with_ip()?;

        let response_code = match request.command {
            Some(Command::Register(r)) => {
                match self.app_state.entities.entry(request.entity_name) {
                    Entry::Occupied(o) => {
                        tracing::error!("Entity {} already registered", o.key());
                        Code::Error
                    }
                    Entry::Vacant(v) => {
                        tracing::info!("Registering entity {}", v.key());
                        match self.open_back_channel(ip, r.port) {
                            Ok(req) => {
                                v.insert(Entity::new(req));
                                Code::Ok
                            }
                            Err(e) => {
                                tracing::error!(error=%e, "Failed to create back-channel: {e:#}");
                                Code::Error
                            }
                        }
                    }
                }
            }
            Some(Command::Unregister(())) => {
                todo!()
            }
            Some(Command::Heartbeat(())) => {
                todo!()
            }
            None => anyhow::bail!("EntityDiscoveryCommand is missing the command"),
        };

        let response = ResponseCode {
            code: response_code.into(),
        };
        self.server.send(response)?;
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
