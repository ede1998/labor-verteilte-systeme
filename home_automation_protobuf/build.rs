use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(&["protobuf/wipmate.proto"], &["protobuf/"])?;
    Ok(())
}
