use anyhow::{Context, Result};
use home_automation_common::{zmq_sockets, OpenTelemetryConfiguration};

use crate::{network::SystemStateRefresher, ui::BackgroundTaskState};

mod network;
mod ui;
mod utility;

fn main() -> Result<()> {
    let log_file = create_log_file()?;
    let _config = OpenTelemetryConfiguration::with_writer("client", log_file)?;
    tracing::info_span!("main").in_scope(|| {
        tracing::info!("Starting client");
        let context = zmq_sockets::Context::new();
        let (sender, receiver) = std::sync::mpsc::channel();
        let refresher = SystemStateRefresher::new(&context, sender)?;
        let handle = refresher.run()?;
        // let client = zmq_sockets::Requester::new(&context)?.connect("tcp://localhost:5556")?;

        let result = ui::run(BackgroundTaskState {
            refresher: &refresher,
            receiver,
        });
        handle
            .join()
            .map_err(|e| anyhow::anyhow!("Refresher task panicked: {e:?}"))?
            .context("Refresher task failed")?;
        result
    })
}

fn create_log_file() -> Result<std::fs::File> {
    use time::format_description::well_known::Iso8601;
    let time = time::OffsetDateTime::now_utc()
        .format(&Iso8601::DEFAULT)
        .context("Failed to format timestamp")?;
    let log_file_name = format!("client-{time}.log");
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_name)
        .with_context(|| anyhow::anyhow!("Failed to open log file: {log_file_name}"))
}
