fn main() {
    println!("cargo:rerun-if-env-changed=DEF_OPENAPI_URL");
    println!("cargo:rerun-if-env-changed=DEF_STREAM_URL");
}
