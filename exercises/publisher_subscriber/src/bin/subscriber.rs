use std::time::Duration;

use anyhow::Context;
use bytes::Bytes;
use prost::Message;
use publisher_subscriber::protobuf::Measurement;

fn main() -> anyhow::Result<()> {
    let topics: Vec<String> = std::env::args().skip(1).collect();

    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB)?;

    subscriber
        .connect("tcp://localhost:8100")
        .context("Failed to connect socket")?;

    for topic in topics {
        subscriber
            .set_subscribe(topic.as_bytes())
            .with_context(|| format!("Failed to subscribe to topic {topic}"))?;
    }

    loop {
        let topic = subscriber
            .recv_string(0)
            .context("Failed to receive topic")?
            .or_else(String::from_utf8)
            .context("Failed to decode string")?;
        let bytes: Bytes = subscriber
            .recv_bytes(0)
            .context("Failed to receive payload")?
            .into();
        let measurement = Measurement::decode(bytes).context("Failed to decode message")?;
        println!("Received {measurement:?} from {topic}");

        std::thread::sleep(Duration::from_millis(1000));
    }
}
