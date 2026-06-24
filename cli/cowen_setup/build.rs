use std::path::PathBuf;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let bin_dir = PathBuf::from(&manifest_dir).join("..").join("..").join("bin").join("windows-x86_64");
    
    // We only tell Cargo to rerun if the directory changes, to avoid constantly rebuilding 
    // unless the files actually appear/disappear.
    println!("cargo:rerun-if-changed={}", bin_dir.display());
    
    let exe_path = bin_dir.join("libcowen_search_embedding.exe");
    let mcp_path = bin_dir.join("cowen-mcp-plugin.exe");
    
    if exe_path.exists() {
        println!("cargo:rustc-cfg=has_search_exe");
    }
    if mcp_path.exists() {
        println!("cargo:rustc-cfg=has_mcp_exe");
    }
}
