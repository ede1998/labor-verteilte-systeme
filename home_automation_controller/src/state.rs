use std::{sync::Mutex, time::Instant};

use dashmap::DashMap;
use home_automation_common::{
    protobuf::{ActuatorState, SensorMeasurement},
    zmq_sockets::{self, markers::Linked},
};

#[derive(Debug, Default)]
pub struct AppState {
    pub entities: DashMap<String, Entity>,
    pub context: zmq_sockets::Context,
}

#[derive(Debug)]
pub struct Entity {
    pub state: Option<EntityState>,
    pub last_heartbeat_pulse: Instant,
    pub connection: Mutex<zmq_sockets::Requester<Linked>>,
}

impl Entity {
    pub fn new(connection: zmq_sockets::Requester<Linked>) -> Self {
        Self {
            state: None,
            last_heartbeat_pulse: Instant::now(),
            connection: connection.into(),
        }
    }
}

#[derive(Debug)]
pub enum EntityState {
    Sensor(SensorMeasurement),
    Actuator(ActuatorState),
}
