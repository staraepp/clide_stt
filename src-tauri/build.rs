fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        link_swift_runtime();
        stage_metal_library();
    }
    stamp_build_info();

    tauri_build::build()
}

/// Put the precompiled Metal library where whisper.cpp looks for it.
///
/// Loading the embedded Metal *source* makes the first dictation compile every
/// kernel at runtime — measured at 11.3 s on an M2 Pro. A precompiled
/// `default.metallib` loads in milliseconds. The packaged app gets it from
/// `Contents/Resources`; a `cargo run`/test binary gets this copy beside it.
fn stage_metal_library() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let source = manifest.join("resources/default.metallib");
    if !source.is_file() {
        println!(
            "cargo:warning=resources/default.metallib is missing; Metal will compile at runtime"
        );
        return;
    }

    // The test binary lives under OUT_DIR; the app binary lives in the profile
    // directory above Cargo's `build` directory.
    let _ = std::fs::copy(&source, out.join("default.metallib"));
    let mut cursor = out.as_path();
    while let Some(parent) = cursor.parent() {
        if cursor.file_name().is_some_and(|name| name == "build") {
            let _ = std::fs::copy(&source, parent.join("default.metallib"));
            break;
        }
        cursor = parent;
    }
    println!("cargo:rerun-if-changed=resources/default.metallib");
}

/// Put the Swift back-deployment runtime on the link path.
///
/// `foundation-models` compiles a small Swift shim to reach Apple
/// Intelligence. In a debug build that shim links
/// `@rpath/libswift_Concurrency.dylib`, which on this SDK exists only in the
/// Xcode toolchain's back-deployment directory. Without this rpath the binary
/// links fine and then **aborts at launch** with a dyld error listing fifty
/// paths and naming no cause.
///
/// Release builds resolve the same symbol to
/// `/usr/lib/swift/libswift_Concurrency.dylib`, which ships in the dyld shared
/// cache on every macOS 26 machine — so a *shipped* app never needs Xcode, and
/// baking a developer's Xcode path into a distributed binary would be wrong.
/// Hence the profile check.
///
/// Resolved through `xcode-select` rather than hardcoded, so a relocated Xcode
/// still works. Missing entirely is not fatal: the linker error that follows is
/// clearer than anything printed here.
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
    // Rebuild when HEAD moves, so the stamp does not go stale. In a normal
    // checkout `.git/HEAD` only contains `ref: refs/heads/<branch>` and does
    // not itself change on every commit, so watch the resolved branch ref too.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    if let Ok(output) = std::process::Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()
    {
        if output.status.success() {
            let reference = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !reference.is_empty() {
                if let Ok(path) = std::process::Command::new("git")
                    .args(["rev-parse", "--git-path", &reference])
                    .output()
                {
                    if path.status.success() {
                        let path = String::from_utf8_lossy(&path.stdout).trim().to_string();
                        if !path.is_empty() {
                            println!("cargo:rerun-if-changed={path}");
                        }
                    }
                }
            }
        }
    }
    println!("cargo:rerun-if-changed=../.git/packed-refs");
}
