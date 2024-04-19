use anyhow::Context;
use entity_discovery::EntityDiscoveryTask;
use state::AppState;

mod entity_discovery;
mod state;

fn main() -> anyhow::Result<()> {
    let _config = home_automation_common::OpenTelemetryConfiguration::new("controller")?;
    let app_state = AppState::default();
    home_automation_common::install_signal_handler(app_state.context.clone())?;
    std::thread::scope(|s| {
        let discovery = s.spawn(|| EntityDiscoveryTask::new(&app_state)?.run());

        discovery
            .join()
            .map_err(|e| anyhow::anyhow!("Entity discovery task panicked: {e:?}"))?
            .context("Entity discovery task failed")?;
        Ok(())
    })
}
