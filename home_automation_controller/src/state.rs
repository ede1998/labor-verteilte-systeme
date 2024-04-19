use dashmap::DashMap;
use home_automation_common::zmq_sockets;

#[derive(Debug, Default)]
pub struct AppState {
    pub sensors: DashMap<String, ()>,
    pub actuators: DashMap<String, ()>,
    pub context: zmq_sockets::Context,
}
