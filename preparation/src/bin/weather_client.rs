/*!
 * Weather update client
 * Connects SUB socket to tcp://localhost:5556
 * Collects weather updates and find avg temp in zip code
 */

use std::env;

fn main() {
    println!("Collecting updates from weather server...");

    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).unwrap();
    assert!(subscriber.connect("tcp://localhost:5556").is_ok());

    let args: Vec<String> = env::args().collect();
    let filter = if args.len() > 1 { &args[1] } else { "10001" };
    assert!(subscriber.set_subscribe(filter.as_bytes()).is_ok());

    let mut total_temp = 0;

    for _ in 0..100 {
        let string = subscriber.recv_string(0).unwrap().unwrap();
        let chunks: Vec<i64> = string.split(' ').map(|num| num.parse().unwrap()).collect();
        let (_zip_code, temperature, _rel_humidity) = (chunks[0], chunks[1], chunks[2]);
        total_temp += temperature;
    }

    println!(
        "Average temperature for zip code '{}' was {}F",
        filter,
        (total_temp / 100)
    );
}
