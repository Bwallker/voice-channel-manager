//! Build script for the voice-channel-manager project.

use std::env::var;

fn main() {
    // trigger recompilation when a new migration is added
    println!("cargo:rerun-if-changed=migrations");

    if let Ok(rust_toolchain) = var("RUSTUP_TOOLCHAIN") {
        if rust_toolchain.contains("stable") {
            // do nothing
        } else if rust_toolchain.contains("nightly") {
            // enable the 'nightly-features' feature flag
            println!("cargo:rustc-cfg=feature=\"nightly-features\"");
        } else if rust_toolchain.contains("beta") {
            println!("cargo:rustc-cfg=feature=\"beta-features\"");
        } else {
            panic!("Unexpected value for rustc toolchain: {rust_toolchain}");
        }
    }
}
