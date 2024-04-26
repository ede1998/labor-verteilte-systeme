use home_automation_actuator::{App, Entity};
use home_automation_common::{
    actuator_state_topic,
    protobuf::{entity_discovery_command::EntityType, ActuatorState},
};

#[derive(Debug)]
struct Actuator;

impl Entity for Actuator {
    fn create_name(base_name: &str) -> String {
        format!("act_{base_name}")
    }

    type PublishData = ActuatorState;

    fn create_initial_data() -> Self::PublishData {
        ActuatorState::default()
    }

    const ENTITY_TYPE: EntityType = EntityType::Actuator;

    fn topic_name(base_name: &str) -> String {
        actuator_state_topic(base_name)
    }

    type UpdateData = ActuatorState;

    fn handle_incoming_data(this: &App<Self>, data: Self::UpdateData) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        todo!()
    }
}

fn main() -> anyhow::Result<()> {
    let app = App::<Actuator>::new()?;
    let _config = home_automation_common::OpenTelemetryConfiguration::new(&app.name)?;

    let sockets = app.connect()?;
    app.run(sockets)
}
