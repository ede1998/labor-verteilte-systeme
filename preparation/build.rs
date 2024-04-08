use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(&["src/bin/protobuf/items.proto"], &["src/"])?;
    Ok(())
}
