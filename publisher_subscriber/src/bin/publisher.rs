use std::time::Duration;

use anyhow::Context;
use prost::Message;
use publisher_subscriber::protobuf::{Measurement, TemperatureUnit};
use rand::{rngs::ThreadRng, Rng};

fn main() -> anyhow::Result<()> {
    let sensor_name = std::env::args()
        .nth(1)
        .context("missing argument 'sensor_name'")?;
    let context = zmq::Context::new();
    let publisher = context.socket(zmq::PUB)?;

    publisher
        .connect("tcp://localhost:5556")
        .context("failed to connect publisher")?;

    let mut rng = rand::thread_rng();

    loop {
        let measurement = random_measurement(&mut rng);
        println!("Sensor {sensor_name} is sending {measurement:?}");

        let buffer = measurement.encode_to_vec();
        publisher
            .send_multipart([&sensor_name], zmq::SNDMORE)
            .context("Failed to send topic name")?;
        publisher
            .send(buffer, 0)
            .context("Failed to send measurement data")?;

        std::thread::sleep(Duration::from_millis(rng.gen_range(100..10000)));
    }
}

fn random_measurement(rng: &mut ThreadRng) -> Measurement {
    let unit: TemperatureUnit = rng.gen();
    let value = rng.gen_range(-100.0..1000.0);

    let mut message = Measurement {
        value,
        ..Default::default()
    };
    message.set_unit(unit);
    message
}
