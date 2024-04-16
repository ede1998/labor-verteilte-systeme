use std::time::Duration;

use anyhow::Context;
use bytes::Bytes;
use home_automation_common::protobuf::{
    entity_discovery_command::{Command, EntityType},
    EntityDiscoveryCommand, ResponseCode,
};
use prost::Message;

fn main() -> anyhow::Result<()> {
    let context = zmq::Context::new();
    let client = context.socket(zmq::REQ)?;

    client
        .connect("tcp://localhost:5556")
        .context("Failed to connect socket")?;

    loop {
        let request = EntityDiscoveryCommand {
            command: Command::Register.into(),
            entity_name: "asd".to_owned(),
            entity_type: EntityType::Sensor.into(),
        };
        let buffer = request.encode_to_vec();
        client.send(buffer, 0)?;

        let bytes: Bytes = client.recv_bytes(0)?.into();
        let measurement = ResponseCode::decode(bytes).context("Failed to decode message")?;
        println!("Received {measurement:?}");

        std::thread::sleep(Duration::from_millis(1000));
    }
}
