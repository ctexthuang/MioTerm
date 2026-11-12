//! Windows: the icon as resource 1, ConPTY beside each executable.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-changed=assets/icons");
    println!("cargo:rerun-if-changed=vendor/conpty");
    println!("cargo:rerun-if-changed=src/ssh/transport.rs");
    println!("cargo:rerun-if-changed=src/ssh/environment.rs");
    embed_resource::compile(
        "assets/icons/elyterminal.rc",
        embed_resource::ParamsIncludeDirs(["assets/icons"]),
    )
    .manifest_required()
    .expect("build: the icon did not go into the Windows executable");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        conpty();
    }
}

/// Copies the target's `conpty.dll` and `OpenConsole.exe` where its executables land.
fn conpty() {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("build: cargo names no target arch");
    let from = PathBuf::from("vendor/conpty").join(&arch);
    let out = PathBuf::from(env::var("OUT_DIR").expect("build: cargo names no OUT_DIR"));
    let transport = transport(&out);
    let profile = out
        .ancestors()
        .nth(3)
        .filter(|_| out.ancestors().nth(2).and_then(|b| b.file_name()) == Some("build".as_ref()))
        .unwrap_or_else(|| {
            panic!(
                "build: OUT_DIR {} is not under target/PROFILE/build",
                out.display()
            )
        });
    for dir in [
        profile.to_path_buf(),
        profile.join("deps"),
        profile.join("examples"),
    ] {
        fs::create_dir_all(&dir)
            .unwrap_or_else(|e| panic!("build: {} not made: {e}", dir.display()));
        for file in ["conpty.dll", "OpenConsole.exe"] {
            fs::copy(from.join(file), dir.join(file)).unwrap_or_else(|e| {
                panic!(
                    "build: {file} for {arch} not copied to {}: {e}",
                    dir.display()
                )
            });
        }
        fs::copy(&transport, dir.join("mioterm-ssh.exe"))
            .unwrap_or_else(|e| panic!("build: SSH transport helper not copied: {e}"));
    }
}

fn transport(out: &Path) -> PathBuf {
    let path = out.join("mioterm-ssh.exe");
    let status = Command::new(env::var_os("RUSTC").expect("build: cargo names no rustc"))
        .args(["--edition=2024", "--crate-name", "mioterm_ssh", "--target"])
        .arg(env::var_os("TARGET").expect("build: cargo names no target"))
        .args(["-C", "opt-level=s", "-C", "target-feature=+crt-static"])
        .arg("src/ssh/transport.rs")
        .arg("-o")
        .arg(&path)
        .status()
        .expect("build: cannot compile the SSH transport helper");
    assert!(
        status.success(),
        "build: SSH transport helper compilation failed"
    );
    path
}
