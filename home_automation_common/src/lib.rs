use anyhow::Context;
use bytes::Bytes;
use opentelemetry_http::{HttpError, Request, Response};
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
            .with_http_client(UReqHttpClient)
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
