//! Build script for the voice-channel-manager project.

use std::env::var;

fn main() {
  // trigger recompilation when a new migration is added
  println!("cargo:rerun-if-changed=migrations");

  let rust_toolchain = var("RUSTUP_TOOLCHAIN",).unwrap();

  if rust_toolchain.starts_with("stable",) {
    // do nothing
  } else if rust_toolchain.starts_with("nightly",) {
    // enable the 'nightly-features' feature flag
    println!("cargo:rustc-cfg=feature=\"nightly-features\"");
  } else if rust_toolchain.starts_with("beta",) {
    println!("cargo:rustc-cfg=feature=\"beta-features\"");
  } else {
    panic!("Unexpected value for rustc toolchain")
  }
}
