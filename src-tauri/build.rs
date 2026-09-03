fn main() {
    link_swift_runtime();

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
