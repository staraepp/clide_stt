# Clide cross-compilation fix

Vendored whisper-rs-sys 0.15.0 from crates.io, retaining its licenses and C++
sources. One build.rs condition uses TARGET instead of cfg!(target_os), which
otherwise tests the build-script host and erroneously links ggml-blas when
cross-compiling from macOS to Windows. Native builds retain their behavior.
Remove this patch when an upstream release includes the correction.
