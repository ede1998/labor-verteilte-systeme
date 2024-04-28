use std::{str::FromStr, time::Duration};

use anyhow::{Context as _, Result};
use home_automation_common::{
    protobuf::{
        entity_discovery_command::EntityType, named_entity_state::State as NState,
        sensor_measurement::Value, HumiditySensorMeasurement, NamedEntityState, SensorMeasurement,
        TemperatureSensorMeasurement,
    },
    sensor_measurement_topic,
};
use home_automation_entity::{App, Entity};
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SensorKind {
    Humidity,
    Temperature,
}

impl SensorKind {
    const ALL: [SensorKind; 2] = [Self::Humidity, Self::Temperature];

    fn list_allowed() -> impl std::fmt::Display {
        struct Printer;
        impl std::fmt::Display for Printer {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "Allowed values: ")?;
                match &SensorKind::ALL[..] {
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

    fn random(self) -> SensorMeasurement {
        let mut rng = rand::thread_rng();
        match self {
            SensorKind::Humidity => SensorMeasurement {
                unit: "%".to_owned(),
                value: Some(Value::Humidity(HumiditySensorMeasurement {
                    humidity: rng.gen_range(0.0..100.0),
                })),
            },
            SensorKind::Temperature => SensorMeasurement {
                unit: "°C".to_owned(),
                value: Some(Value::Temperature(TemperatureSensorMeasurement {
                    temperature: rng.gen_range(-40.0..45.0),
                })),
            },
        }
    }
}

impl FromStr for SensorKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        let pos = Self::ALL
            .map(|kind| kind.to_string())
            .iter()
            .position(|kind| kind == s)
            .with_context(|| format!("Unknown sensor kind {s}. {}", Self::list_allowed()))?;
        Ok(Self::ALL[pos])
    }
}

impl std::fmt::Display for SensorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SensorKind::Humidity => f.write_str(stringify!(Humidity)),
            SensorKind::Temperature => f.write_str(stringify!(Temperature)),
        }
    }
}

#[derive(Debug)]
struct Sensor {
    topic: String,
    name: String,
    data_kind: SensorKind,
}

impl Entity for Sensor {
    const ENTITY_TYPE: EntityType = EntityType::Sensor;

    fn new(base_name: String) -> Result<Self> {
        let kind: SensorKind = std::env::args()
            .nth(2)
            .with_context(|| format!("Missing sensor kind. {} ", SensorKind::list_allowed()))?
            .parse()?;

        Ok(Self {
            topic: sensor_measurement_topic(&base_name),
            name: format!("sen_{base_name}"),
            data_kind: kind,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn topic_name(&self) -> &str {
        &self.topic
    }

    type PublishData = SensorMeasurement;

    fn retrieve_publish_data(&self) -> Self::PublishData {
        self.data_kind.random()
    }

    fn handle_incoming_data(&self, data: NamedEntityState) -> Result<Option<Duration>> {
        anyhow::ensure!(
            data.entity_name == self.name,
            "Message arrived at wrong sensor. Expected {} but got {}",
            data.entity_name,
            self.name
        );
        match data.state {
            Some(NState::SensorConfiguration(config)) => Ok(Some(Duration::from_secs_f32(
                1. / config.update_frequency_hz,
            ))),
            None => Err(anyhow::anyhow!("Missing payload data in {:?}", data.state)),
            Some(other) => Err(anyhow::anyhow!("Invalid payload for sensor: {other:?}",)),
        }
    }
}

fn main() -> Result<()> {
    let app = App::<Sensor>::new()?;
    let _config = home_automation_common::OpenTelemetryConfiguration::new(app.entity.name())?;

    let sockets = app.connect()?;
    app.run(sockets)
}
