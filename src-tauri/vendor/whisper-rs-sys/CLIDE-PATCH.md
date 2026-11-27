# Clide patches

Vendored whisper-rs-sys 0.15.0 from crates.io, retaining its licenses and C++
sources.

- One `build.rs` condition uses `TARGET` instead of `cfg!(target_os)`, which
  otherwise tests the build-script host and erroneously links ggml-blas when
  cross-compiling from macOS to Windows. Native builds retain their behavior.
- The Metal backend defaults to a precompiled `default.metallib` instead of
  embedding the Metal source. The embedded source path compiled every kernel on
  first launch (about 11 seconds on an M2 Pro); the precompiled library loads in
  milliseconds. `CLIDE_METAL_EMBED_LIBRARY=1` restores the old behavior for
  experiments.
- The Metal install rule no longer points at a nonexistent
  `src/ggml-metal/ggml-metal.metal` path when the library is not embedded.

Remove these patches when upstream includes the corrections.
