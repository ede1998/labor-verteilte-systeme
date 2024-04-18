use anyhow::Context;
use dashmap::DashMap;
use home_automation_common::{
    protobuf::{entity_discovery_command, response_code, EntityDiscoveryCommand, ResponseCode},
    shutdown_requested, zmq_sockets,
};

#[derive(Debug, Default)]
struct AppState {
    sensors: DashMap<String, ()>,
    actuators: DashMap<String, ()>,
    context: zmq_sockets::Context,
}

fn main() -> anyhow::Result<()> {
    let _config = home_automation_common::OpenTelemetryConfiguration::new("controller")?;
    let app_state = AppState::default();
    home_automation_common::install_signal_handler(app_state.context.clone())?;
    std::thread::scope(|s| {
        let discovery = s.spawn(|| EntityDiscoveryTask::new(&app_state)?.run());

        discovery
            .join()
            .map_err(|e| anyhow::anyhow!("Entity discovery task panicked: {e:?}"))?
            .context("Entity discovery task failed")?;
        Ok(())
    })
}

struct EntityDiscoveryTask<'a> {
    app_state: &'a AppState,
    server: zmq_sockets::Replier<zmq_sockets::markers::Linked>,
}

impl<'a> EntityDiscoveryTask<'a> {
    fn new(app_state: &'a AppState) -> anyhow::Result<Self> {
        let address = "tcp://*:5556";
        let server = zmq_sockets::Replier::new(&app_state.context)?.bind(address)?;
        Ok(Self { app_state, server })
    }

    #[tracing::instrument(skip(self))]
    fn run(&self) -> anyhow::Result<()> {
        while !shutdown_requested() {
            let _ = self.accept_entity();
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    fn accept_entity(&self) -> anyhow::Result<()> {
        use dashmap::mapref::entry::Entry;
        use entity_discovery_command::Command;
        use response_code::Code;
        let request: EntityDiscoveryCommand = self.server.receive()?;

        let response_code = match request.command() {
            Command::Register => match self.app_state.actuators.entry(request.entity_name) {
                Entry::Occupied(o) => {
                    tracing::error!("Actuator {} already registered", o.key());
                    Code::Error
                }
                Entry::Vacant(v) => {
                    tracing::info!("Registering actuator {}", v.key());
                    v.insert(());
                    Code::Ok
                }
            },
            Command::Unregister => {
                todo!()
            }
            Command::Heartbeat => {
                todo!()
            }
        };

        let response = ResponseCode {
            code: response_code.into(),
        };
        self.server.send(response)?;
        Ok(())
    }
}
