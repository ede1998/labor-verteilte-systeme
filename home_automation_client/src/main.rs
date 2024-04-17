use std::time::Duration;

use home_automation_common::{
    protobuf::{
        entity_discovery_command::{Command, EntityType},
        EntityDiscoveryCommand, ResponseCode,
    },
    shutdown_requested, zmq_sockets, OpenTelemetryConfiguration,
};

fn main() -> anyhow::Result<()> {
    let _config = OpenTelemetryConfiguration::new("client")?;
    tracing::info_span!("main").in_scope(|| {
        tracing::info!("Starting controller");
        let context = zmq_sockets::Context::new();
        let client = zmq_sockets::Requester::new(&context)?.connect("tcp://localhost:5556")?;

        while !shutdown_requested() {
            let _ = send_entity(&client);
            std::thread::sleep(Duration::from_millis(1000));
        }
        Ok(())
    })
}

#[tracing::instrument(parent=None, skip(client), err)]
fn send_entity(
    client: &zmq_sockets::Requester<zmq_sockets::markers::Linked>,
) -> anyhow::Result<()> {
    let request = EntityDiscoveryCommand {
        command: Command::Register.into(),
        entity_name: "asd".to_owned(),
        entity_type: EntityType::Sensor.into(),
    };

    tracing::debug!("Sending {request:?}");
    client.send(request)?;

    let response_code: ResponseCode = client.receive()?;
    tracing::debug!("Received {response_code:?}");
    Ok(())
}
