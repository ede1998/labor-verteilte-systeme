use anyhow::Context;
use bytes::Bytes;
use home_automation_common::{protobuf::EntityDiscoveryCommand, shutdown_requested};
use prost::Message;

fn main() -> anyhow::Result<()> {
    let _config = home_automation_common::OpenTelemetryConfiguration::new("controller")?;
    tracing::info_span!("main").in_scope(|| {
        tracing::info!("Starting controller");
        entity_discovery().context("Job entity_discovery failed")?;
        Ok(())
    })
}

#[tracing::instrument]
fn entity_discovery() -> anyhow::Result<()> {
    let context = zmq::Context::new();
    let server = context.socket(zmq::REP)?;

    let address = "tcp://*:5556";
    server
        .bind(address)
        .with_context(|| format!("Failed to bind address {address}"))?;

    while !shutdown_requested() {
        let _ = accept_entity(&server);
    }
    Ok(())
}

#[tracing::instrument(skip(server), err)]
fn accept_entity(server: &zmq::Socket) -> anyhow::Result<()> {
    let bytes: Bytes = server.recv_bytes(0)?.into();
    let request = EntityDiscoveryCommand::decode(bytes)?;
    tracing::debug!("Received {request:?}");
    // let measurement = random_measurement(&mut rng);
    // let buffer = measurement.encode_to_vec();
    // server.send(buffer, 0)?;
    Ok(())
}
