use std::time::{Duration, Instant};

use home_automation_common::{shutdown_requested, HEARTBEAT_FREQUENCY};

use crate::state::AppState;

pub struct TimeoutTask<'a> {
    app_state: &'a AppState,
}

impl<'a> TimeoutTask<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self { app_state }
    }

    #[tracing::instrument(name = "Timeout for un-registration", skip(self))]
    pub fn run(&self) -> anyhow::Result<()> {
        tracing::info!("Running Timeout task.");
        let mut last_run = Instant::now();
        while !shutdown_requested() {
            std::thread::sleep(Duration::from_millis(100));
            if last_run.elapsed() > HEARTBEAT_FREQUENCY {
                self.unregister_dead_entities();
                last_run = Instant::now();
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn unregister_dead_entities(&self) {
        let now = Instant::now();
        self.app_state.entities.retain(|name, entity| {
            if now.duration_since(entity.last_heartbeat_pulse) < HEARTBEAT_FREQUENCY * 2 {
                true
            } else {
                tracing::info!("Unregistering entity {name} because of missed heartbeats");
                false
            }
        });
    }
}
