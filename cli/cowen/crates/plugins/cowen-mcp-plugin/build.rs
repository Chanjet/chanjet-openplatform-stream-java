fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path().unwrap());
    tonic_build::compile_protos("../../../proto/native_api_registry.proto")?;
    tonic_build::compile_protos("../../../proto/public_system.proto")?;
    Ok(())
}
