// build.rs
// Linker script to compile Zig libghostty and link static archives directly to Rust executable.
use std::env;
use std::fs;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target = env::var("TARGET").unwrap();

    println!("cargo:rerun-if-changed=vendor/libghostty");

    // Ensure target OUT_DIR exists
    fs::create_dir_all(&out_dir).expect("Failed to create OUT_DIR");

    let out_lib = format!("{}/libghostty.a", out_dir);

    // 1. Invoke Zig compiler to build static archive for libghostty C-ABI
    if target.contains("apple") {
        let obj_file = format!("{}/ghostty.o", out_dir);

        let zig_output = Command::new("zig")
            .args(&[
                "build-obj",
                "vendor/libghostty/src/lib.zig",
                "-O", "ReleaseFast",
                "-lc",
                &format!("-femit-bin={}", obj_file),
            ])
            .output()
            .expect("Failed to execute Zig compiler");

        if !zig_output.status.success() {
            panic!(
                "Zig build-obj failed:\nStdout: {}\nStderr: {}",
                String::from_utf8_lossy(&zig_output.stdout),
                String::from_utf8_lossy(&zig_output.stderr)
            );
        }

        // Apple ld requires 8-byte aligned Mach-O archives, created cleanly via libtool
        let libtool_output = Command::new("libtool")
            .args(&["-static", "-o", &out_lib, &obj_file])
            .output()
            .expect("Failed to execute libtool");

        if !libtool_output.status.success() {
            panic!(
                "libtool static archive packaging failed:\nStdout: {}\nStderr: {}",
                String::from_utf8_lossy(&libtool_output.stdout),
                String::from_utf8_lossy(&libtool_output.stderr)
            );
        }
    } else {
        let zig_output = Command::new("zig")
            .args(&[
                "build-lib",
                "vendor/libghostty/src/lib.zig",
                "-O", "ReleaseFast",
                "--name", "ghostty",
                "-lc",
                &format!("-femit-bin={}", out_lib),
            ])
            .output()
            .expect("Failed to execute Zig compiler");

        if !zig_output.status.success() {
            panic!(
                "Zig build-lib failed:\nStdout: {}\nStderr: {}",
                String::from_utf8_lossy(&zig_output.stdout),
                String::from_utf8_lossy(&zig_output.stderr)
            );
        }
    }

    // 2. Instruct Cargo to search and bind the static lib
    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=ghostty");

    // 3. Platform-specific framework linkages
    if target.contains("apple") {
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=CoreText");
    } else if target.contains("windows") {
        println!("cargo:rustc-link-lib=dylib=gdi32");
        println!("cargo:rustc-link-lib=dylib=user32");
    } else {
        println!("cargo:rustc-link-lib=dylib=fontconfig");
        println!("cargo:rustc-link-lib=dylib=freetype");
    }
}
