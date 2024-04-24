fn main() {}
/*
use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let actuator_name = std::env::args().skip(1).next().context("Missing actuator name.")?;
    let actuator_name = format!("act_{actuator_name}");
    let _config = home_automation_common::OpenTelemetryConfiguration::new(actuator_name)?;
    let context = zmq_sockets::Context::new();
    home_automation_common::install_signal_handler(app_state.context.clone())?;
    std::thread::scope(|s| {
        let discovery = s.spawn(|| EntityDiscoveryTask::new(&app_estate)?.run());
        let client_api = s.spawn(|| ClientApiTask::new(&app_state)?.run());

        discovery
            .join()
            .map_err(|e| anyhow::anyhow!("Entity discovery task panicked: {e:?}"))?
            .context("Entity discovery task failed")?;
        client_api
            .join()
            .map_err(|e| anyhow::anyhow!("Client API task panicked: {e:?}"))?
            .context("Client API task failed")?;
        Ok(())
    })
}
*/