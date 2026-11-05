use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let ghostty_dir = workspace_root.join("vendor").join("libghostty");

    println!("cargo:rerun-if-changed={}", ghostty_dir.join("src/lib.zig").display());
    println!("cargo:rerun-if-changed={}", ghostty_dir.join("include/libghostty.h").display());

    let zig_out_lib = ghostty_dir.join("zig-out").join("lib");
    let archive_path = zig_out_lib.join("libghostty.a");

    if !archive_path.exists() {
        let _ = Command::new("zig")
            .current_dir(&ghostty_dir)
            .args(["build", "-Doptimize=ReleaseFast"])
            .status();
    }

    if archive_path.exists() {
        println!("cargo:rustc-link-search=native={}", zig_out_lib.display());
        println!("cargo:rustc-link-lib=static=ghostty");
    }
}

