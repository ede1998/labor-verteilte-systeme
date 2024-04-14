use anyhow::Context;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub mod protobuf {
    include!(concat!(env!("OUT_DIR"), "/wipmate.rs"));
}

pub struct OpenTelemetryConfiguration(());

impl OpenTelemetryConfiguration {
    pub fn new(service_name: impl Into<String>) -> anyhow::Result<Self> {
        opentelemetry::global::set_text_map_propagator(opentelemetry_zipkin::Propagator::new());

        let tracer = opentelemetry_zipkin::new_pipeline()
            .with_service_name(service_name)
            .install_simple()
            .context("Failed to install opentelemetry_zipkin tracer")?;

        let tracer = tracing_opentelemetry::layer().with_tracer(tracer);

        let subscriber = tracing_subscriber::fmt::layer().json();

        tracing_subscriber::registry()
            .with(subscriber)
            .with(EnvFilter::from_default_env())
            .with(tracer)
            .init();

        Ok(OpenTelemetryConfiguration(()))
    }
}

impl Drop for OpenTelemetryConfiguration {
    fn drop(&mut self) {
        opentelemetry::global::shutdown_tracer_provider();
    }
}
