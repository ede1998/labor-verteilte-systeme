//! Temperature sensor server
//! Binds REP socket to tcp://*:5556

use anyhow::Context;
use bytes::Bytes;
use client_server::protobuf::{Measurement, Request, TemperatureUnit};
use prost::Message;
use rand::{rngs::ThreadRng, Rng};

fn main() -> anyhow::Result<()> {
    let context = zmq::Context::new();
    let server = context.socket(zmq::REP)?;

    server.bind("tcp://*:5556")?;

    let mut rng = rand::thread_rng();

    loop {
        let bytes: Bytes = server.recv_bytes(0)?.into();
        let request = Request::decode(bytes).context("Failed to decode message")?;
        println!("Received {request:?}");
        let measurement = random_measurement(&mut rng);
        let buffer = measurement.encode_to_vec();
        server.send(buffer, 0)?;
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
