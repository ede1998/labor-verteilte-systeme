use anyhow::Context as _;
use home_automation_common::{
    load_env,
    protobuf::{
        client_api_command::CommandType, entity_discovery_command::EntityType, ClientApiCommand,
        NamedEntityState, ResponseCode, SystemState,
    },
    shutdown_requested,
    zmq_sockets::{self, markers::Linked, termination_is_ok},
};

use crate::state::AppState;

pub struct ClientApiTask<'a> {
    app_state: &'a AppState,
    server: zmq_sockets::Replier<Linked>,
}

impl<'a> ClientApiTask<'a> {
    pub fn new(app_state: &'a AppState) -> anyhow::Result<Self> {
        let address = load_env(home_automation_common::ENV_CLIENT_API_ENDPOINT)?;
        let server = zmq_sockets::Replier::new(&app_state.context)?.bind(&address)?;
        Ok(Self { app_state, server })
    }

    #[tracing::instrument(name = "Client Api", skip(self))]
    pub fn run(&self) -> anyhow::Result<()> {
        tracing::info!("Starting Client API.");
        while !shutdown_requested() {
            let Err(e) = self.handle_client() else {
                continue;
            };
            return Err(e).or_else(termination_is_ok);
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn handle_client(&self) -> anyhow::Result<()> {
        let request: ClientApiCommand = self.server.receive()?;
        match request.command_type {
            Some(CommandType::Query(_)) => {
                self.handle_system_state_query()?;
            }
            Some(CommandType::Action(entity_state)) => {
                let result = self.handle_entity_state_command(entity_state);
                tracing::info!(
                    ?result,
                    "Handled NamedEntityState command with result: {result:?}"
                );
                let response_code: ResponseCode = result.into();
                self.server.send(response_code)?;
            }
            None => {
                tracing::error!("Failed to handle request: Missing command in ClientApiCommand.");
                let response_code: ResponseCode =
                    Err::<(), _>(anyhow::anyhow!("Missing command in ClientApiCommand")).into();
                self.server.send(response_code)?;
            }
        }

        Ok(())
    }

    fn handle_system_state_query(&self) -> anyhow::Result<()> {
        let system_state = {
            use crate::state::EntityState;
            use std::collections::HashMap;

            let mut sensors = HashMap::new();
            let mut actuators = HashMap::new();
            let mut new_sensors = Vec::new();
            let mut new_actuators = Vec::new();

            for entity_entry in &self.app_state.entities {
                let (name, state) = entity_entry.pair();
                match &state.state {
                    EntityState::Sensor(measurement) => {
                        sensors.insert(name.to_owned(), measurement.clone());
                    }
                    EntityState::Actuator(state) => {
                        actuators.insert(name.to_owned(), state.clone());
                    }
                    EntityState::New(EntityType::Sensor) => new_sensors.push(name.to_owned()),
                    EntityState::New(EntityType::Actuator) => new_actuators.push(name.to_owned()),
                }
            }

            SystemState {
                sensors,
                actuators,
                new_sensors,
                new_actuators,
            }
        };

        tracing::debug!(?system_state, "Prepared system state response for sending.");

        self.server
            .send(system_state)
            .context("Failed to send system state response")
    }

    fn handle_entity_state_command(&self, entity_state: NamedEntityState) -> anyhow::Result<()> {
        use home_automation_common::protobuf::response_code::Code;
        let entity_name = entity_state.entity_name.clone();

        let entity = self.app_state.entities.get(&entity_name).with_context(|| {
            anyhow::anyhow!(
                "Unknown entity {} in NamedEntityState command",
                &entity_state.entity_name
            )
        })?;

        let response_code: ResponseCode = {
            tracing::debug!(?entity_state, "Forwarding command via back-channel.");
            let connection = entity.connection.lock().expect("poisoned mutex");

            connection.send(entity_state)?;
            connection.receive()?
        };

        match response_code.code() {
            Code::Ok => Ok(()),
            Code::Error => Err(anyhow::anyhow!("Failed to update entity {entity_name}")),
        }
    }
}
