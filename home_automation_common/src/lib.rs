use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use anyhow::Context;
use bytes::Bytes;
use opentelemetry_http::{HttpError, Request, Response};
use protobuf::entity_discovery_command::EntityType;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub trait AnyhowExt<T> {
    fn erase_err(self) -> anyhow::Result<T>;
}

impl<T, E> AnyhowExt<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn erase_err(self) -> anyhow::Result<T> {
        self.map_err(Into::into)
    }
}

pub trait AnyhowZmq {
    fn is_zmq_termination(&self) -> bool;
}

impl AnyhowZmq for anyhow::Error {
    fn is_zmq_termination(&self) -> bool {
        self.downcast_ref()
            .is_some_and(|e: &zmq::Error| matches!(e, zmq::Error::ETERM))
    }
}

pub mod zmq_sockets;

pub mod protobuf {
    include!(concat!(env!("OUT_DIR"), "/wipmate.rs"));

    impl<T, E> From<Result<T, E>> for ResponseCode {
        fn from(value: Result<T, E>) -> Self {
            ResponseCode {
                code: match value {
                    Ok(_) => response_code::Code::Ok,
                    Err(_) => response_code::Code::Error,
                }
                .into(),
            }
        }
    }

    impl From<SensorMeasurement> for PublishData {
        fn from(m: SensorMeasurement) -> Self {
            Self {
                value: Some(publish_data::Value::Measurement(m)),
            }
        }
    }

    impl From<ActuatorState> for PublishData {
        fn from(m: ActuatorState) -> Self {
            Self {
                value: Some(publish_data::Value::ActuatorState(m)),
            }
        }
    }

    impl ActuatorState {
        pub fn light(brightness: f32) -> Self {
            Self {
                state: Some(actuator_state::State::Light(LightActuatorState {
                    brightness,
                })),
            }
        }

        pub fn air_conditioning(on: bool) -> Self {
            Self {
                state: Some(actuator_state::State::AirConditioning(
                    AirConditioningActuatorState { on },
                )),
            }
        }
    }

    impl NamedEntityState {
        pub fn actuator(entity_name: impl Into<String>, value: ActuatorState) -> Self {
            Self {
                entity_name: entity_name.into(),
                state: Some(named_entity_state::State::ActuatorState(value)),
            }
        }

        pub fn frequency(entity_name: impl Into<String>, update_frequency_hz: f32) -> Self {
            Self {
                entity_name: entity_name.into(),
                state: Some(named_entity_state::State::SensorConfiguration(
                    SensorConfiguration {
                        update_frequency_hz,
                    },
                )),
            }
        }
    }

    impl std::fmt::Display for entity_discovery_command::EntityType {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str(match self {
                Self::Actuator => "Actuator",
                Self::Sensor => "Sensor",
            })
        }
    }

    impl ClientApiCommand {
        pub fn system_state_query() -> Self {
            use client_api_command::CommandType;
            ClientApiCommand {
                command_type: Some(CommandType::Query(SystemStateQuery::default())),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum EntityState {
    Sensor(protobuf::SensorMeasurement),
    Actuator(protobuf::ActuatorState),
    New(EntityType),
}

impl EntityState {
    pub fn entity_type(&self) -> EntityType {
        match self {
            Self::Sensor(_) => EntityType::Sensor,
            Self::Actuator(_) => EntityType::Actuator,
            Self::New(t) => *t,
        }
    }
}

pub const ENV_DISCOVERY_ENDPOINT: &str = "HOME_AUTOMATION_DISCOVERY_ENDPOINT";
pub const ENV_ENTITY_DATA_ENDPOINT: &str = "HOME_AUTOMATION_ENTITY_DATA_ENDPOINT";
pub const ENV_CLIENT_API_ENDPOINT: &str = "HOME_AUTOMATION_CLIENT_API_ENDPOINT";

pub fn load_env(var: &str) -> anyhow::Result<String> {
    std::env::var(var).with_context(|| anyhow::anyhow!("Failed to read env var {var}"))
}

pub const HEARTBEAT_FREQUENCY: Duration = Duration::from_secs(10);

pub fn actuator_name(topic: &str) -> anyhow::Result<String> {
    Ok(topic
        .strip_prefix("/actuator_state/")
        .with_context(|| anyhow::anyhow!("Failed to parse topic {topic} as actuator topic"))?
        .to_owned())
}

pub fn actuator_state_topic(name: &str) -> String {
    format!("/actuator_state/{name}")
}

pub fn sensor_name(topic: &str) -> anyhow::Result<String> {
    Ok(topic
        .strip_prefix("/measurement/")
        .with_context(|| anyhow::anyhow!("Failed to parse topic {topic} as sensor topic"))?
        .to_owned())
}
pub fn sensor_measurement_topic(name: &str) -> String {
    format!("/measurement/{name}")
}

pub fn entity_topic(name: &str, entity_type: EntityType) -> String {
    match entity_type {
        EntityType::Actuator => actuator_state_topic(name),
        EntityType::Sensor => sensor_measurement_topic(name),
    }
}

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

#[inline]
pub fn shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

#[inline]
pub fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn install_signal_handler(mut context: zmq_sockets::Context) -> anyhow::Result<()> {
    ctrlc::set_handler(move || {
        tracing::info!("Shutdown signal received");
        request_shutdown();
        context.destroy().expect("Failed to destroy context");
    })
    .context("Failed to install signal handler")
}

pub struct OpenTelemetryConfiguration(());

impl OpenTelemetryConfiguration {
    pub fn with_writer<W>(service_name: impl Into<String>, writer: W) -> anyhow::Result<Self>
    where
        W: for<'w> tracing_subscriber::fmt::MakeWriter<'w> + 'static + Send + Sync,
    {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "debug,ureq=info");
        }
        opentelemetry::global::set_text_map_propagator(opentelemetry_zipkin::Propagator::new());

        let tracer = opentelemetry_zipkin::new_pipeline()
            .with_service_name(service_name)
            .with_http_client(UReqHttpClient)
            .install_simple()
            .context("Failed to install opentelemetry_zipkin tracer")?;

        let tracer = tracing_opentelemetry::layer().with_tracer(tracer);

        let subscriber = tracing_subscriber::fmt::layer().with_writer(writer);

        tracing_subscriber::registry()
            .with(subscriber)
            .with(EnvFilter::from_default_env())
            .with(tracer)
            .init();

        Ok(OpenTelemetryConfiguration(()))
    }
    pub fn new(service_name: impl Into<String>) -> anyhow::Result<Self> {
        Self::with_writer(service_name, std::io::stderr)
    }
}

impl Drop for OpenTelemetryConfiguration {
    fn drop(&mut self) {
        opentelemetry::global::shutdown_tracer_provider();
    }
}

#[derive(Debug)]
struct UReqHttpClient;

#[async_trait::async_trait]
impl opentelemetry_http::HttpClient for UReqHttpClient {
    async fn send(&self, request: Request<Vec<u8>>) -> Result<Response<Bytes>, HttpError> {
        let (http_parts, body) = request.into_parts();
        let ureq_request: ureq::Request = http_parts.into();
        let ureq_response = ureq_request.send_bytes(&body)?;
        let response: opentelemetry_http::Response<Vec<u8>> = ureq_response.into();
        Ok(response.map(bytes::Bytes::from))
    }
}
