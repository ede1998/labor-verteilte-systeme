use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(&["protobuf/measurement.proto"], &["protobuf/"])?;
    Ok(())
}
