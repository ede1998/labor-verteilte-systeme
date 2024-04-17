use std::io::Result;

fn main() -> Result<()> {
    prost_build::Config::new()
        .enable_type_names()
        .compile_protos(&["protobuf/wipmate.proto"], &["protobuf/"])
}
