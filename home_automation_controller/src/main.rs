use anyhow::Context;
use home_automation_common::{
    protobuf::{response_code, EntityDiscoveryCommand, ResponseCode},
    shutdown_requested, zmq_sockets,
};

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
    let context = zmq_sockets::Context::new();
    let address = "tcp://*:5556";
    let server = zmq_sockets::Replier::new(&context)?.bind(address)?;

    while !shutdown_requested() {
        let _ = accept_entity(&server);
    }
    Ok(())
}

#[tracing::instrument(skip(server), err)]
fn accept_entity(
    server: &zmq_sockets::Replier<zmq_sockets::markers::Linked>,
) -> anyhow::Result<()> {
    let request: EntityDiscoveryCommand = server.receive()?;
    tracing::debug!("Received {request:?}");
    let response = ResponseCode {
        code: response_code::Code::Ok.into(),
    };
    server.send(response)?;
    Ok(())
}
