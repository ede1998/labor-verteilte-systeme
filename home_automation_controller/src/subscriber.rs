use anyhow::Context as _;
use home_automation_common::{
    load_env,
    protobuf::{publish_data, PublishData},
    shutdown_requested,
    zmq_sockets::{self, markers::Linked},
    AnyhowZmq, EntityState,
};

use crate::state::AppState;

pub struct SubscriberTask<'a> {
    app_state: &'a AppState,
    subscriber: zmq_sockets::Subscriber<Linked>,
}

impl<'a> SubscriberTask<'a> {
    pub fn new(app_state: &'a AppState) -> anyhow::Result<Self> {
        let address = load_env(home_automation_common::ENV_ENTITY_DATA_ENDPOINT)?;
        let subscriber = zmq_sockets::Subscriber::new(&app_state.context)?.bind(&address)?;
        subscriber.subscribe("")?;
        Ok(Self {
            app_state,
            subscriber,
        })
    }

    #[tracing::instrument(name = "Subscriber", skip(self))]
    pub fn run(&self) -> anyhow::Result<()> {
        tracing::info!("Starting Subscriber.");
        while !shutdown_requested() {
            self.handle_client();
        }
        Ok(())
    }

    #[tracing::instrument(name = "receive sample", skip(self))]
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
        let (topic, payload): (String, PublishData) = self.subscriber.receive()?;

        let update_state = |name, state| -> anyhow::Result<()> {
            let mut entry = self.app_state.entities.get_mut(&name).with_context(|| {
                anyhow::anyhow!("Payload {state:?} received for unknown entity {name}")
            })?;
            tracing::info!("Updating entity {name} with new state {state:?}");
            entry.state = state;
            Ok(())
        };

        match payload.value {
            None => anyhow::bail!("Missing payload in {payload:?} for topic {topic}"),
            Some(publish_data::Value::Measurement(m)) => {
                let name = home_automation_common::sensor_name(&topic)?;
                update_state(name, EntityState::Sensor(m))?;
            }
            Some(publish_data::Value::ActuatorState(s)) => {
                let name = home_automation_common::actuator_name(&topic)?;
                update_state(name, EntityState::Actuator(s))?;
            }
        }
        Ok(())
    }
}
