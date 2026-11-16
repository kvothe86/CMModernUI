// A user's own local additions (src/local/mod.rs, not in the repository) are built in when the
// folder is there; CMUI_PUBLIC=1 leaves them out of a build that is shared.
fn main() {
    println!("cargo::rustc-check-cfg=cfg(cmui_local)");
    println!("cargo:rerun-if-changed=src/local");
    println!("cargo:rerun-if-env-changed=CMUI_PUBLIC");
    if std::path::Path::new("src/local/mod.rs").exists() && std::env::var_os("CMUI_PUBLIC").is_none() {
        println!("cargo:rustc-cfg=cmui_local");
    }
}
