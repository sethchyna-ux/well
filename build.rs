// build.rs
// Linker script to compile Zig libghostty and link static archives directly to Rust executable.
use std::env;
use std::fs;
use std::process::Command;

fn rust_target_to_zig_target(target: &str) -> Option<&'static str> {
    if target.contains("x86_64-unknown-linux-musl") {
        Some("x86_64-linux-musl")
    } else if target.contains("x86_64-unknown-linux") {
        Some("x86_64-linux-gnu")
    } else if target.contains("aarch64-unknown-linux-musl") {
        Some("aarch64-linux-musl")
    } else if target.contains("aarch64-unknown-linux") {
        Some("aarch64-linux-gnu")
    } else if target.contains("x86_64-pc-windows") {
        Some("x86_64-windows-gnu")
    } else if target.contains("aarch64-apple-darwin") {
        Some("aarch64-macos")
    } else if target.contains("x86_64-apple-darwin") {
        Some("x86_64-macos")
    } else {
        None
    }
}

fn main() {
    let out_dir = match env::var("OUT_DIR") {
        Ok(dir) => dir,
        Err(_) => return,
    };
    let target = env::var("TARGET").unwrap_or_default();

    println!("cargo:rerun-if-changed=vendor/libghostty");

    let _ = fs::create_dir_all(&out_dir);
    let out_lib = format!("{}/libghostty.a", out_dir);

    let zig_target = rust_target_to_zig_target(&target);

    // 1. Invoke Zig compiler to build static archive for libghostty C-ABI
    if target.contains("apple") {
        let obj_file = format!("{}/ghostty.o", out_dir);

        let mut cmd = Command::new("zig");
        cmd.args(&[
            "build-obj",
            "vendor/libghostty/src/lib.zig",
            "-O",
            "ReleaseFast",
            "-lc",
            &format!("-femit-bin={}", obj_file),
        ]);
        if let Some(zt) = zig_target {
            cmd.arg("-target").arg(zt);
        }

        match cmd.output() {
            Ok(output) if output.status.success() => {
                let libtool_res = Command::new("libtool")
                    .args(&["-static", "-o", &out_lib, &obj_file])
                    .output();
                match libtool_res {
                    Ok(lo) if lo.status.success() => {
                        println!("cargo:rustc-link-search=native={}", out_dir);
                        println!("cargo:rustc-link-lib=static=ghostty");
                    }
                    Ok(lo) => {
                        eprintln!(
                            "cargo:warning=libtool failed: {}",
                            String::from_utf8_lossy(&lo.stderr)
                        );
                    }
                    Err(e) => {
                        eprintln!("cargo:warning=libtool execution error: {}", e);
                    }
                }
            }
            Ok(output) => {
                eprintln!(
                    "cargo:warning=Zig build-obj failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(e) => {
                eprintln!("cargo:warning=Zig not found: {}", e);
            }
        }
    } else {
        let mut cmd = Command::new("zig");
        cmd.args(&[
            "build-lib",
            "vendor/libghostty/src/lib.zig",
            "-O",
            "ReleaseFast",
            "--name",
            "ghostty",
            "-lc",
            &format!("-femit-bin={}", out_lib),
        ]);

        if let Some(zt) = zig_target {
            cmd.arg("-target").arg(zt);
        }

        match cmd.output() {
            Ok(output) if output.status.success() => {
                println!("cargo:rustc-link-search=native={}", out_dir);
                println!("cargo:rustc-link-lib=static=ghostty");
            }
            Ok(output) => {
                eprintln!(
                    "cargo:warning=Zig build-lib failed (code {:?}): {}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(e) => {
                eprintln!(
                    "cargo:warning=Zig compiler not found: {}. Skipping vendor libghostty static archive.",
                    e
                );
            }
        }
    }

    // 2. Platform-specific framework linkages
    if target.contains("apple") {
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=CoreText");
    } else if target.contains("windows") {
        println!("cargo:rustc-link-lib=dylib=gdi32");
        println!("cargo:rustc-link-lib=dylib=user32");

        // Generate mingw d3dcompiler import library via Zig's llvm-dlltool
        let def_path = format!("{}/d3dcompiler.def", out_dir);
        let lib_path = format!("{}/libd3dcompiler.a", out_dir);
        let def_content = "LIBRARY d3dcompiler_47.dll\nEXPORTS\nD3DCompile\nD3DDisassemble\nD3DReflect\n";
        let _ = std::fs::write(&def_path, def_content);
        let _ = Command::new("zig")
            .args(&[
                "dlltool",
                "-m",
                "i386:x86-64",
                "-d",
                &def_path,
                "-l",
                &lib_path,
            ])
            .status();
        println!("cargo:rustc-link-search=native={}", out_dir);
    }
}
