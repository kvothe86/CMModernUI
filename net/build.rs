// The exports (forwards to ws2_32 / mswsock and our own cmnet_* functions) from wsock32.def, each
// as a linker /EXPORT: a second /DEF beside rustc's own loses the forwards.
fn main() {
    let def = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("wsock32.def");
    println!("cargo:rerun-if-changed=wsock32.def");
    // (link.exe makes these exports stubs into the import libraries rather than forwards: the
    // ws2_32 ones find ws2_32.lib, the mswsock ones need mswsock.lib)
    println!("cargo:rustc-link-lib=mswsock");
    for line in std::fs::read_to_string(&def).unwrap().lines().skip(2) {
        // "    bind=cmnet_bind @2" -> /EXPORT:bind=cmnet_bind,@2
        let (entry, ordinal) = line.trim().split_once(" @").unwrap();
        println!("cargo:rustc-cdylib-link-arg=/EXPORT:{entry},@{ordinal}");
    }
}
