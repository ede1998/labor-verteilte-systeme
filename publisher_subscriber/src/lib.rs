use rand::{
    distributions::{Distribution, Standard},
    Rng,
};

pub mod protobuf {
    include!(concat!(env!("OUT_DIR"), "/server_client.rs"));
}

impl Distribution<protobuf::TemperatureUnit> for Standard {
    fn sample<R>(&self, rng: &mut R) -> protobuf::TemperatureUnit
    where
        R: Rng + ?Sized,
    {
        match rng.gen_range(0..=2) {
            0 => protobuf::TemperatureUnit::Celsius,
            1 => protobuf::TemperatureUnit::Kelvin,
            _ => protobuf::TemperatureUnit::Fahrenheit,
        }
    }
}
