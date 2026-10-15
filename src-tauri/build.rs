fn main() {
    // Parakeet runs on ONNX Runtime, which ships no build for Intel Macs, so the
    // Intel half of the universal macOS app leaves Parakeet out. Code checks
    // `#[cfg(parakeet)]`; Cargo.toml drops the dependency for the same target.
    println!("cargo::rustc-check-cfg=cfg(parakeet)");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if !(os == "macos" && arch == "x86_64") {
        println!("cargo::rustc-cfg=parakeet");
    }
    stamp_build();
    tauri_build::build()
}

/// Sets WORDFLOW_BUILD to the release tag this build came from, e.g.
/// "v2.0.0-alpha.8", or "v2.0.0-alpha.8-3-g86cd117" for 3 commits past it, so
/// Settings shows exactly which build is installed. Empty without git.
fn stamp_build() {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let build = git(&["describe", "--tags", "--always", "--match", "v2.*"]).unwrap_or_default();
    println!("cargo::rustc-env=WORDFLOW_BUILD={build}");
    // Re-stamp after a commit, checkout or new tag.
    for path in ["HEAD", "refs", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", path]) {
            println!("cargo::rerun-if-changed={path}");
        }
    }
}
