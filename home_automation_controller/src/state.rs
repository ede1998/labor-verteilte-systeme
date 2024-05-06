use std::{sync::Mutex, time::Instant};

use anyhow::{Context as _, Result};
use dashmap::DashMap;
use home_automation_common::{
    protobuf::{entity_discovery_command::EntityType, ActuatorState, SensorMeasurement},
    zmq_sockets::{self, markers::Linked},
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

#[derive(Debug)]
pub enum EntityState {
    Sensor(SensorMeasurement),
    Actuator(ActuatorState),
    New(EntityType),
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Subscribe,
    Unsubscribe,
}

#[derive(Debug, Clone)]
pub struct SubscriptionCommand {
    pub topic: String,
    pub action: Action,
}

impl SubscriptionCommand {
    pub fn subscribe(topic: String) -> Self {
        Self {
            topic,
            action: Action::Subscribe,
        }
    }
    pub fn unsubscribe(topic: String) -> Self {
        Self {
            topic,
            action: Action::Unsubscribe,
        }
    }
}
