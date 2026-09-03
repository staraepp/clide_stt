fn main() {
    link_swift_runtime();
    stamp_build_info();

    tauri_build::build()
}

/// Put the Swift back-deployment runtime on the link path.
///
/// `foundation-models` compiles a small Swift shim to reach Apple
/// Intelligence. That shim links `libswift_Concurrency.dylib`, which on this
/// SDK lives only in the Xcode toolchain's back-deployment directory — not in
/// `/usr/lib/swift` and not in the dyld cache. Without this rpath the binary
/// builds fine and then aborts at launch with a dyld error.
///
/// Resolved through `xcode-select` rather than hardcoded, so a different Xcode
/// location still works. Missing entirely is not fatal: the linker error it
/// would cause is clearer than anything printed here.
fn link_swift_runtime() {
    let Ok(output) = std::process::Command::new("xcode-select").arg("-p").output() else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let developer = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let candidate = format!(
        "{developer}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx"
    );

    if std::path::Path::new(&candidate).is_dir() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{candidate}");
    }
}

/// Bake the commit and build date in, so the About panel can name the exact
/// build someone is running when they report a bug.
///
/// A missing git checkout is not an error — a tarball build still works, it
/// just reports "unknown".
fn stamp_build_info() {
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".into());

    println!("cargo:rustc-env=CLIDE_COMMIT={commit}");
    println!(
        "cargo:rustc-env=CLIDE_BUILD_DATE={}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default()
    );
    // Rebuild when HEAD moves, so the stamp does not go stale.
    println!("cargo:rerun-if-changed=../.git/HEAD");
}
