fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure().compile_protos(
        &[
            "../../../proto/native_audit.proto",
            "../../../proto/native_auth.proto",
            "../../../proto/native_config.proto",
            "../../../proto/native_dlq.proto",
            "../../../proto/native_system.proto",
            "../../../proto/native_worker.proto",
            "../../../proto/public_system.proto",
        ],
        &["../../../proto"],
    )?;
    tonic_build::compile_protos("../../../proto/native_api_registry.proto")?;
    Ok(())
}
