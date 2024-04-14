use tracing::Level;

fn main() -> anyhow::Result<()> {
    let _config = home_automation_common::OpenTelemetryConfiguration::new("controller")?;

    let span = tracing::span!(Level::ERROR, "my first span");
    let _enter = span.enter();
    tracing::info!("Starting controller");
    tracing::error!("Is this the default log level?");

    Ok(())
}
