use std::{sync::Mutex, time::Instant};

use anyhow::{Context as _, Result};
use dashmap::DashMap;
use home_automation_common::{
    protobuf::entity_discovery_command::EntityType,
    zmq_sockets::{self, markers::Linked},
    EntityState,
};

#[derive(Debug, Default)]
pub struct AppState {
    pub entities: DashMap<String, Entity>,
    pub context: zmq_sockets::Context,
}

impl AppState {
    pub fn unregister(&self, entity_name: &str) -> Result<()> {
        self.entities
            .remove(entity_name)
            .with_context(|| anyhow::anyhow!("Failed to remove unknown entity {entity_name}"))?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Entity {
    pub state: EntityState,
    pub last_heartbeat_pulse: Instant,
    pub connection: Mutex<zmq_sockets::Requester<Linked>>,
}

impl Entity {
    pub fn new(connection: zmq_sockets::Requester<Linked>, entity_type: EntityType) -> Self {
        Self {
            state: EntityState::New(entity_type),
            last_heartbeat_pulse: Instant::now(),
            connection: connection.into(),
        }
    }
}
