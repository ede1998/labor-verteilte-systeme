use anyhow::Context as _;
use home_automation_common::{
    load_env,
    protobuf::{entity_discovery_command, response_code, ResponseCode},
    shutdown_requested,
    zmq_sockets::{self, markers::Linked},
};

use crate::state::{AppState, Entity};

pub struct ClientApiTask<'a> {
    app_state: &'a AppState,
    server: zmq_sockets::Replier<Linked>,
}

impl<'a> ClientApiTask<'a> {
    pub fn new(app_state: &'a AppState) -> anyhow::Result<Self> {
        let address = load_env(home_automation_common::ENV_CLIENT_API_ENDPOINT)?;
        let server = zmq_sockets::Replier::new(&app_state.context)?.bind(&address)?;
        Ok(Self { app_state, server })
    }

    #[tracing::instrument(name = "Client Api", skip(self))]
    pub fn run(&self) -> anyhow::Result<()> {
        tracing::info!("Starting Client API.");
        self.handle_client()?;
        return Ok(());
        while !shutdown_requested() {
            let _ = self.handle_client();
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    fn handle_client(&self) -> anyhow::Result<()> {
        use dashmap::mapref::entry::Entry;
        use entity_discovery_command::Command;
        use response_code::Code;
        // let request: ClientApiCommand = self.server.receive()?;

        let entity = loop {
            if let Some(e) = self.app_state.entities.iter().next() {
                break e;
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        };

        tracing::info!("sending request via back-channel");

        let response = ResponseCode {
            code: Code::Ok.into(),
        };
        entity
            .connection
            .lock()
            .expect("poisoned mutex")
            .send(response)?;

        // self.server.send(response)?;
        Ok(())
    }
}
