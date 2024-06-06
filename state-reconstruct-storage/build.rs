
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = prost_build::Config::new();
    config.protoc_arg("--experimental_allow_proto3_optional=true");
    config.compile_protos(&["proto/snapshot.proto"], &["proto/"])?;
    Ok(())
}
