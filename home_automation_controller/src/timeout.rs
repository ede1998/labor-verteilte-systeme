use std::{
    sync::mpsc::Sender,
    time::{Duration, Instant},
};

use home_automation_common::{shutdown_requested, HEARTBEAT_FREQUENCY};

use crate::state::{AppState, SubscriptionCommand};

pub struct TimeoutTask<'a> {
    app_state: &'a AppState,
    unregistering_queue: Sender<SubscriptionCommand>,
}

impl<'a> TimeoutTask<'a> {
    pub fn new(app_state: &'a AppState, unregistering_queue: Sender<SubscriptionCommand>) -> Self {
        Self {
            app_state,
            unregistering_queue,
        }
    }

    #[tracing::instrument(name = "Timeout for un-registration", skip(self))]
    pub fn run(&self) -> anyhow::Result<()> {
        tracing::info!("Running Timeout task.");
        let mut last_run = Instant::now();
        while !shutdown_requested() {
            std::thread::sleep(Duration::from_millis(100));
            if last_run.elapsed() > HEARTBEAT_FREQUENCY {
                self.unregister_dead_entities()?;
                last_run = Instant::now();
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn unregister_dead_entities(&self) -> anyhow::Result<()> {
        let now = Instant::now();
        let mut result = Ok(());
        self.app_state.entities.retain(|name, entity| {
            if result.is_err() {
                return true;
            }
            if now.duration_since(entity.last_heartbeat_pulse) < HEARTBEAT_FREQUENCY * 2 {
                return true;
            }

            tracing::info!("Unregistering entity {name} because of missed heartbeats");

            result = self
                .unregistering_queue
                .send(SubscriptionCommand::unsubscribe(
                    home_automation_common::entity_topic(name, entity.state.entity_type()),
                ));

            false
        });

        result.map_err(Into::into)
    }
}
