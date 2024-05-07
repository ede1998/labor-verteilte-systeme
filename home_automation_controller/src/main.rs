use anyhow::Context;
use client_api::ClientApiTask;
use entity_discovery::EntityDiscoveryTask;
use state::AppState;
use subscriber::SubscriberTask;
use timeout::TimeoutTask;

mod client_api;
mod entity_discovery;
mod state;
mod subscriber;
mod timeout;

fn main() -> anyhow::Result<()> {
    let _config = home_automation_common::OpenTelemetryConfiguration::new("controller")?;
    let app_state = AppState::default();
    home_automation_common::install_signal_handler(app_state.context.clone())?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::scope(|s| {
        let discovery = s.spawn({
            let tx = tx.clone();
            || EntityDiscoveryTask::new(&app_state, tx)?.run()
        });
        let client_api = s.spawn(|| ClientApiTask::new(&app_state)?.run());
        let subscriber = s.spawn(|| SubscriberTask::new(&app_state)?.run(rx));
        let timeout = s.spawn(|| TimeoutTask::new(&app_state, tx).run());

        discovery
            .join()
            .map_err(|e| anyhow::anyhow!("Entity discovery task panicked: {e:?}"))?
            .context("Entity discovery task failed")?;
        subscriber
            .join()
            .map_err(|e| anyhow::anyhow!("Subscriber task panicked: {e:?}"))?
            .context("Subscriber task failed")?;
        client_api
            .join()
            .map_err(|e| anyhow::anyhow!("Client API task panicked: {e:?}"))?
            .context("Client API task failed")?;
        timeout
            .join()
            .map_err(|e| anyhow::anyhow!("Timeout task panicked: {e:?}"))?
            .context("Timeout task failed")?;
        Ok(())
    })
}
