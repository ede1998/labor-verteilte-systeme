use home_automation_actuator::App;

fn main() -> anyhow::Result<()> {
    let app = App::new()?;
    let _config = home_automation_common::OpenTelemetryConfiguration::new(&app.name)?;

    let sockets = app.connect()?;
    app.run(sockets)
}
