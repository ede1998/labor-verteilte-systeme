//! Temperature controller client
//! Connects with REQ socket to tcp://*:5556

use std::time::Duration;

use anyhow::Context;
use bytes::Bytes;
use client_server::protobuf::{Measurement, Request};
use prost::Message;

fn main() -> anyhow::Result<()> {
    let context = zmq::Context::new();
    let client = context.socket(zmq::REQ)?;

    client
        .connect("tcp://localhost:5556")
        .context("Failed to connect socket")?;

    loop {
        let request = Request {};
        let buffer = request.encode_to_vec();
        client.send(buffer, 0)?;

        let bytes: Bytes = client.recv_bytes(0)?.into();
        let measurement = Measurement::decode(bytes).context("Failed to decode message")?;
        println!("Received {measurement:?}");

        std::thread::sleep(Duration::from_millis(1000));
    }
}
