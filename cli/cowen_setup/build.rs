use std::path::PathBuf;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let dll_path = PathBuf::from(&manifest_dir).join("..").join("..").join("bin").join("windows-x86_64").join("cowen_search_embedding.dll");
    
    // We only tell Cargo to rerun if the directory changes, to avoid constantly rebuilding 
    // unless the files actually appear/disappear.
    println!("cargo:rerun-if-changed={}", dll_path.parent().unwrap().display());
    
    if dll_path.exists() {
        println!("cargo:rustc-cfg=has_ai_plugin");
    }
}
