pub mod snazzy {
    pub mod items {
        include!(concat!(env!("OUT_DIR"), "/snazzy.items.rs"));
    }
}

use std::{
    fs::File,
    io::{ErrorKind, Write},
};

use anyhow::Context;
use bytes::{BufMut, BytesMut};
use prost::Message;
use snazzy::items;

pub fn create_large_shirt(color: String) -> items::Shirt {
    let mut shirt = items::Shirt {
        color,
        ..Default::default()
    };
    shirt.set_size(items::shirt::Size::Large);
    shirt
}

fn main() -> anyhow::Result<()> {
    let shirt = match File::open("shirt.protobin") {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            println!("File not found, creating new shirt");
            create_large_shirt("Green".to_string())
        }
        Err(e) => anyhow::bail!(e),
        Ok(mut file) => {
            let mut data = BytesMut::new().writer();
            std::io::copy(&mut file, &mut data).context("Failed to read file")?;
            items::Shirt::decode(data.into_inner()).context("Failed to decode message")?
        }
    };
    println!("{shirt:#?}");
    let mut buffer = BytesMut::new();
    shirt
        .encode(&mut buffer)
        .context("Failed to encode message")?;
    let mut file = File::create("shirt.protobin").context("Failed to create file")?;
    file.write_all(&buffer[..])
        .context("Failed to write file")?;

    Ok(())
}
