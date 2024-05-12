use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, mpsc::Sender, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use anyhow::Result;
use home_automation_common::{
    load_env,
    zmq_sockets::{invalid_state_is_ok, markers::Linked, timeout_is_ok, Context, Requester},
    EntityState, ENV_CLIENT_API_ENDPOINT,
};

type State = HashMap<String, EntityState>;
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug)]
struct InnerRefresher {
    sender: Sender<State>,
    requester: Requester<Linked>,
}

impl InnerRefresher {
    #[tracing::instrument(name = "refresh system state", skip(self))]
    fn refresh_once(&mut self) -> Result<()> {
        use home_automation_common::protobuf::{
            entity_discovery_command::EntityType, ClientApiCommand, SystemState,
        };

        let sensor = |(name, measurement)| (name, EntityState::Sensor(measurement));
        let actuator = |(name, state)| (name, EntityState::Actuator(state));
        let new_sensor = |name| (name, EntityState::New(EntityType::Sensor));
        let new_actuator = |name| (name, EntityState::New(EntityType::Actuator));

        let request = ClientApiCommand::system_state_query();
        self.requester.send(request).or_else(invalid_state_is_ok)?;
        let response: SystemState = self.requester.receive()?;
        tracing::info!("Constructing local system state");
        let sensors = response.sensors.into_iter().map(sensor);
        let actuators = response.actuators.into_iter().map(actuator);
        let new_sensors = response.new_sensors.into_iter().map(new_sensor);
        let new_actuators = response.new_actuators.into_iter().map(new_actuator);
        let state = sensors
            .chain(actuators)
            .chain(new_sensors)
            .chain(new_actuators)
            .collect();
        tracing::info!(?state, "Sending new state to UI");
        self.sender.send(state)?;
        Ok(())
    }

    fn task(mut self, auto_refresh: Arc<AtomicBool>) -> Result<()> {
        tracing::info!("Starting refresh task");
        while !home_automation_common::shutdown_requested() {
            self.refresh_once().or_else(timeout_is_ok)?;

            if home_automation_common::shutdown_requested() {
                break;
            }
            tracing::debug!("Parking refresh thread");
            if auto_refresh.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::park_timeout(REFRESH_INTERVAL);
            } else {
                std::thread::park();
            }
            tracing::debug!("Unparked refresh thread");
        }

        tracing::info!("Shutdown of refresher thread");

        Ok(())
    }
}

#[derive(Debug)]
enum ThreadState {
    StartPending(InnerRefresher),
    Running(std::thread::Thread),
}

#[derive(Debug)]
pub struct SystemStateRefresher {
    inner: Mutex<ThreadState>,
    auto_refresh: Arc<AtomicBool>,
}

impl SystemStateRefresher {
    pub fn new(context: &Context, sender: Sender<State>) -> Result<Self> {
        let mut requester =
            Requester::new(context)?.connect(&load_env(ENV_CLIENT_API_ENDPOINT)?)?;
        requester.set_message_exchange_timeout(Some(Duration::from_millis(800)))?;
        Ok(Self {
            inner: Mutex::new(ThreadState::StartPending(InnerRefresher {
                sender,
                requester,
            })),
            auto_refresh: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn toggle_auto_refresh(&self) {
        use std::sync::atomic::Ordering;
        // invert the value by using value XOR true
        let current_value = !self.auto_refresh.fetch_xor(true, Ordering::SeqCst);
        if current_value {
            tracing::info!("Auto-refresh activated");
            self.refresh();
        } else {
            tracing::info!("Auto-refresh deactivated");
        }
    }

    pub fn refresh(&self) {
        let mut guard = self.inner.lock().expect("non-poisoned Mutex");
        if let ThreadState::Running(thread) = &mut *guard {
            thread.unpark();
        }
    }

    pub fn run(&self) -> Result<JoinHandle<Result<()>>> {
        let auto_refresh = self.auto_refresh.clone();
        let mut guard = self.inner.lock().expect("non-poisoned mutex");

        // get ownership and replace with dummy value until done
        match std::mem::replace(&mut *guard, ThreadState::Running(std::thread::current())) {
            ThreadState::Running(thread) => {
                *guard = ThreadState::Running(thread);
                Err(anyhow::anyhow!("Thread already started"))
            }
            ThreadState::StartPending(inner) => {
                let handle = std::thread::spawn(move || inner.task(auto_refresh));
                *guard = ThreadState::Running(handle.thread().clone());

                Ok(handle)
            }
        }
    }
}
