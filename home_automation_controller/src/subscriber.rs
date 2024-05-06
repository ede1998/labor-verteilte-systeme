use std::{
    sync::{mpsc::Receiver, Mutex},
    time::Duration,
};

use anyhow::Context as _;
use home_automation_common::{
    load_env,
    protobuf::{publish_data, PublishData},
    shutdown_requested,
    zmq_sockets::{self, markers::Linked},
    AnyhowZmq,
};

use crate::state::{Action, AppState, SubscriptionCommand};

pub struct SubscriberTask<'a> {
    app_state: &'a AppState,
    subscriber: Mutex<zmq_sockets::Subscriber<Linked>>,
}

impl<'a> SubscriberTask<'a> {
    pub fn new(app_state: &'a AppState) -> anyhow::Result<Self> {
        let address = load_env(home_automation_common::ENV_ENTITY_DATA_ENDPOINT)?;
        let subscriber = zmq_sockets::Subscriber::new(&app_state.context)?.bind(&address)?;
        Ok(Self {
            app_state,
            subscriber: Mutex::new(subscriber),
        })
    }

    #[tracing::instrument(name = "Subscriber", skip(self))]
    pub fn run(&self, rx: Receiver<SubscriptionCommand>) -> anyhow::Result<()> {
        tracing::info!("Starting Subscriber.");
        std::thread::scope(|s| {
            s.spawn(move || {
                while !shutdown_requested() {
                    let Ok(request) = rx.recv_timeout(Duration::from_millis(100)) else {
                        continue;
                    };
                    tracing::debug!("Updating subscription: {request:?}");
                    let result = {
                        let socket = self.subscriber.lock().expect("non-poisoned Mutex");
                        match request.action {
                            Action::Subscribe => socket.subscribe(request.topic),
                            Action::Unsubscribe => socket.unsubscribe(request.topic),
                        }
                    };
                    if let Err(e) = result {
                        if !e.is_zmq_termination() {
                            tracing::error!("Failed to update subscription: {e:#}");
                        }
                    }
                }
            });

            while !shutdown_requested() {
                self.handle_client();
            }
        });
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn handle_client(&self) {
        let result = self.inner_handle_client();
        if let Err(e) = result {
            if !e.is_zmq_termination() {
                tracing::error!("Failed handle client publication: {e:#}");
            } else {
                tracing::info!("Cannot handle client publication because shutdown is in progress.");
            }
        }
    }

    fn inner_handle_client(&self) -> anyhow::Result<()> {
        let subscriber = self.subscriber.lock().expect("non-poisoned Mutex");
        let (topic, payload): (String, PublishData) = subscriber.receive()?;

        let update_state = |name, state| -> anyhow::Result<()> {
            let mut entry = self.app_state.entities.get_mut(&name).with_context(|| {
                anyhow::anyhow!("Payload {state:?} received for unknown entity {name}")
            })?;
            entry.state = state;
            Ok(())
        };

        match payload.value {
            None => anyhow::bail!("Missing payload in {payload:?} for topic {topic}"),
            Some(publish_data::Value::Measurement(m)) => {
                let name = home_automation_common::sensor_name(&topic)?;
                update_state(name, crate::state::EntityState::Sensor(m))?;
            }
            Some(publish_data::Value::ActuatorState(s)) => {
                let name = home_automation_common::actuator_name(&topic)?;
                update_state(name, crate::state::EntityState::Actuator(s))?;
            }
        }
        Ok(())
    }
}
