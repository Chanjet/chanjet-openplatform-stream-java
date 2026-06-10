include!("../../../build_common.rs");

fn main() {
    run_build_script("../../../.git");

    tonic_build::configure()
        .compile_protos(
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
        )
        .expect("Failed to compile native protos");
    tonic_build::compile_protos("../../../proto/native_api_registry.proto")
        .expect("Failed to compile native_api_registry.proto");
}
