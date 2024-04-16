use std::time::Duration;

use home_automation_common::{
    protobuf::{
        entity_discovery_command::{Command, EntityType},
        EntityDiscoveryCommand, ResponseCode,
    },
    zmq_sockets,
};

fn main() -> anyhow::Result<()> {
    let context = zmq_sockets::Context::new();
    let client = zmq_sockets::Requester::new(&context)?.connect("tcp://localhost:5556")?;

    loop {
        let request = EntityDiscoveryCommand {
            command: Command::Register.into(),
            entity_name: "asd".to_owned(),
            entity_type: EntityType::Sensor.into(),
        };
        client.send(request)?;

        let measurement: ResponseCode = client.receive()?;
        println!("Received {measurement:?}");

        std::thread::sleep(Duration::from_millis(1000));
    }
}
