use std::{fs::OpenOptions, time::Duration};

use anyhow::{Context, Result};
use home_automation_common::{
    protobuf::{
        entity_discovery_command::{Command, EntityType, Registration},
        EntityDiscoveryCommand, ResponseCode,
    },
    shutdown_requested, zmq_sockets, OpenTelemetryConfiguration,
};
use time::format_description::well_known::Iso8601;

mod ui;
mod utility;

fn main() -> Result<()> {
    let time = time::OffsetDateTime::now_utc()
        .format(&Iso8601::DEFAULT)
        .context("Failed to format timestamp")?;
    let log_file_name = format!("client-{time}.log");
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_name)
        .with_context(|| anyhow::anyhow!("Failed to open log file: {log_file_name}"))?;
    let _config = OpenTelemetryConfiguration::with_writer("client", log_file)?;
    tracing::info_span!("main").in_scope(|| {
        tracing::info!("Starting client");
        // let context = zmq_sockets::Context::new();
        // let client = zmq_sockets::Requester::new(&context)?.connect("tcp://localhost:5556")?;

        // while !shutdown_requested() {
        //     let _ = send_entity(&context, &client);
        //     std::thread::sleep(Duration::from_millis(1000));
        // }
        ui::run()
    })
}

#[tracing::instrument(parent=None, skip_all, err)]
fn send_entity(
    context: &zmq_sockets::Context,
    client: &zmq_sockets::Requester<zmq_sockets::markers::Linked>,
) -> Result<()> {
    let rep = zmq_sockets::Replier::new(context)?.bind("tcp://*:*")?;
    let ep = rep.get_last_endpoint()?;
    let request = EntityDiscoveryCommand {
        command: Command::Register(Registration {
            port: ep.port().into(),
        })
        .into(),
        entity_name: "asd".to_owned(),
        entity_type: EntityType::Sensor.into(),
    };

    tracing::debug!("Sending {request:?}");
    client.send(request)?;

    let response_code: ResponseCode = client.receive()?;
    tracing::debug!("Received {response_code:?}");

    let response_code: ResponseCode = rep.receive()?;
    tracing::debug!("HALLELUJAH {response_code:?}");

    Ok(())
}
