use std::{str::FromStr, sync::RwLock, time::Duration};

use anyhow::{Context as _, Result};
use home_automation_common::{
    actuator_state_topic,
    protobuf::{
        actuator_state::State, entity_discovery_command::EntityType,
        named_entity_state::State as NState, ActuatorState, AirConditioningActuatorState,
        LightActuatorState, NamedEntityState, PublishData,
    },
};
use home_automation_entity::{App, Entity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActuatorKind {
    AirConditioning,
    Light,
}

impl ActuatorKind {
    const ALL: [ActuatorKind; 2] = [Self::AirConditioning, Self::Light];

    fn list_allowed() -> impl std::fmt::Display {
        struct Printer;
        impl std::fmt::Display for Printer {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "Allowed values: ")?;
                match &ActuatorKind::ALL[..] {
                    [] => {}
                    [one] => write!(f, "{one}")?,
                    [many @ .., last] => {
                        for kind in many {
                            write!(f, "{kind}, ")?;
                        }
                        write!(f, "{last}")?;
                    }
                }

                Ok(())
            }
        }
        Printer
    }
}

impl FromStr for ActuatorKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let pos = Self::ALL
            .map(|kind| kind.to_string())
            .iter()
            .position(|kind| kind == s)
            .with_context(|| format!("Unknown actuator kind {s}. {}", Self::list_allowed()))?;
        Ok(Self::ALL[pos])
    }
}

impl From<ActuatorKind> for State {
    fn from(value: ActuatorKind) -> Self {
        match value {
            ActuatorKind::AirConditioning => {
                State::AirConditioning(AirConditioningActuatorState::default())
            }
            ActuatorKind::Light => State::Light(LightActuatorState::default()),
        }
    }
}

impl From<&State> for ActuatorKind {
    fn from(value: &State) -> Self {
        match value {
            State::Light(_) => Self::Light,
            State::AirConditioning(_) => Self::AirConditioning,
        }
    }
}

impl std::fmt::Display for ActuatorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ActuatorKind::AirConditioning => f.write_str("AirConditioning"),
            ActuatorKind::Light => f.write_str("Light"),
        }
    }
}

#[derive(Debug)]
struct Actuator {
    topic: String,
    name: String,
    data: RwLock<State>,
}

impl Entity for Actuator {
    const ENTITY_TYPE: EntityType = EntityType::Actuator;

    fn new(base_name: String) -> Result<Self> {
        let kind: ActuatorKind = std::env::args()
            .nth(2)
            .with_context(|| format!("Missing actuator kind. {} ", ActuatorKind::list_allowed()))?
            .parse()?;

        Ok(Self {
            topic: actuator_state_topic(&base_name),
            name: format!("act_{base_name}"),
            data: RwLock::new(kind.into()),
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn topic_name(&self) -> &str {
        &self.topic
    }

    fn retrieve_publish_data(&self) -> PublishData {
        let state = self.data.read().expect("non-poisoned RwLock").clone();
        ActuatorState { state: Some(state) }.into()
    }

    fn handle_incoming_data(&self, data: NamedEntityState) -> Result<Option<Duration>> {
        anyhow::ensure!(
            data.entity_name == self.name,
            "Message arrived at wrong actuator. Expected {} but got {}",
            data.entity_name,
            self.name
        );
        match data.state {
            None | Some(NState::ActuatorState(ActuatorState { state: None })) => {
                Err(anyhow::anyhow!("Missing payload data in {:?}", data.state))
            }
            Some(NState::ActuatorState(ActuatorState {
                state: Some(new_state),
            })) => {
                let mut old_state = self.data.write().expect("non-poisoned RwLock");
                let old_kind = ActuatorKind::from(&*old_state);
                let new_kind = ActuatorKind::from(&new_state);
                anyhow::ensure!(
                    old_kind == new_kind,
                    "Incompatible state kind {new_kind} received for {old_kind}"
                );
                *old_state = new_state;
                Ok(None)
            }
            Some(NState::SensorConfiguration(config)) => Ok(Some(Duration::from_secs_f32(
                1. / config.update_frequency_hz,
            ))),
        }
    }
}

fn main() -> Result<()> {
    let app = App::<Actuator>::new()?;
    let _config = home_automation_common::OpenTelemetryConfiguration::new(app.entity.name())?;

    let sockets = app.connect()?;
    app.run(sockets)
}
