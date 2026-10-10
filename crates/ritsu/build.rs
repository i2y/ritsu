//! The `ritsu` built for WASI (the npm package, DESIGN 8.8) gets the stack of a native main thread,
//! 8 MiB, in place of the 1 MiB rustc gives a WebAssembly module: the checks recurse as deep there
//! as they do natively. Here rather than in packaging/npm/build.sh's flags, so that every build for
//! WASI has it, however cargo is run. Nothing is said on any other target.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("wasi") {
        println!("cargo:rustc-link-arg-bins=-zstack-size=8388608");
    }
}
