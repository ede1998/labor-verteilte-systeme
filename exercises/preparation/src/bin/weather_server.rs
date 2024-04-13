//! Weather update server
//! Binds PUB socket to tcp://*:5556 and ipc://weather.ipc
//! Publishes random weather updates

use std::fmt::Write;

use rand::Rng;

fn main() {
    let context = zmq::Context::new();
    let publisher = context.socket(zmq::PUB).unwrap();

    assert!(publisher.bind("tcp://*:5556").is_ok());
    assert!(publisher.bind("ipc://weather.ipc").is_ok());

    let mut rng = rand::thread_rng();
    let mut update = String::with_capacity(20);

    loop {
        let zip_code = rng.gen_range(0..100_000);
        let temperature = rng.gen_range(-80..135);
        let rel_humidity = rng.gen_range(10..60);

        write!(
            &mut update,
            "{:05} {} {}",
            zip_code, temperature, rel_humidity
        )
        .unwrap();
        publisher.send(&update, 0).unwrap();
        update.clear();
    }

    // note: destructors mean no explicit cleanup necessary
}
