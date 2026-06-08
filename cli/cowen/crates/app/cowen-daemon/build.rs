fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("../../../proto/daemon.proto")?;
    tonic_build::compile_protos("../../../proto/api_registry.proto")?;
    Ok(())
}
