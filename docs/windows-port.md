# Clide 2.1.2 Windows port

Windows x64 uses local Whisper and Parakeet. CPU-only builds remain the default;
builds made with the `cuda` Cargo feature accelerate both engines on NVIDIA
GPUs. The capture-time phrase worker, processing/backtracking, dictionary,
history, model downloads and loopback API are shared with macOS. Apple Speech
is excluded. Rewrite is unavailable on Windows; Verbatim and deterministic
Polished remain available.

Native Windows support includes:

- Foreground process identity, without reading app/document contents.
- Unicode SendInput insertion with modifier-release and foreground-PID checks.
  Partial/blocked insertion reports failure rather than retrying text and
  risking duplication. The transcript remains in history/on the clipboard.
- Windows clipboard access, including bounded retries for a busy clipboard.
- Microphone privacy policy checks. Actual capture errors remain authoritative;
  a policy that permits access is not proof that a particular microphone works.
- A non-focusable floating HUD, Windows shortcut labels and onboarding model
  download. A fresh Windows install selects Whisper Base; no model is
  downloaded until the user requests it.
- AppData logs and runtime RAM/CPU detection. CUDA builds also report the
  NVIDIA adapter and use it for Whisper and Parakeet acceleration.

The initial port supports PCM WAV imports. Other imported formats produce an
explicit unsupported-format error. Audio ducking and Apple live typing are
unavailable on Windows. Background phrase recognition is supported instead;
no provisional text is inserted into the editor.

## Build

On Windows, install Rust/MSVC, Node and the normal Tauri prerequisites, then run
`scripts/build-windows.ps1` from the repository root. The script prepares the
runtime directory and produces an NSIS installer. Pass `-CpuOnly` to skip CUDA
auto-detection. The installer requires Microsoft Edge WebView2 Runtime to be
installed already; it does not silently download a runtime.

For GPU acceleration, install the CUDA 13.x toolkit and an NVIDIA driver that
supports the target GPU. With `CUDA_PATH` set or `nvcc.exe` on `PATH`, the script
enables the `cuda` Cargo feature automatically. Use `-CpuOnly` to force the
original CPU build.
The combined CUDA path currently requires CUDA 13 because that is the Windows
CUDA provider shipped by the pinned ONNX Runtime dependency.

The Windows runtime directory contains DirectML and, for CUDA builds, the ONNX
Runtime CUDA provider plus `cudart64_13.dll`, `cublas64_13.dll` and
`cublasLt64_13.dll`. Required Microsoft Visual C++ runtime DLLs are also packaged
beside the executable. `nvcuda.dll` is supplied by the NVIDIA driver and is not
bundled.

`vendor/whisper-rs-sys` contains upstream 0.15.0 with a one-line build-script
correction: native library selection tests the target rather than the build
host. This is needed for a Mac-to-Windows cross-build. Original licenses and
source are retained. See its CLIDE-PATCH.md.

## Validation boundaries

Mac regression suite: 329 passed, six manual tests ignored and the
system-volume-changing test skipped. Frontend production build passed.
Windows x64 MSVC compilation checks and release NSIS cross-build passed for the
CPU package. The CPU installer archive was extracted to verify its x64
executable and required DirectML/MSVC DLLs. This does not test executing the
installer on Windows. The Win64 INPUT ABI size is checked at compile time. A
cross-build is not a native microphone/insertion test.

The CUDA package was built natively on a Windows Server 2022 `g4dn.2xlarge`
(Tesla T4) with CUDA 13.0.48, Visual Studio 2022 Build Tools 17.14, CMake Ninja,
and `CUDAARCHS=75`. `whisper-rs` reused the crate's generated bindings only after
they failed the Windows size assertions; the working build regenerated them with
libclang. The release executable and NSIS installer completed. Live GPU
inference, microphone capture, insertion and installer execution were not run.

That first CUDA artifact is **T4-only**: its `clide.exe` contains only
`sm_75` code. Do not use it on RTX 30/40/50-series cards; they need a compatible
SASS architecture or PTX. `scripts/build-windows.ps1` now defaults to
`CUDAARCHS=all-major` for new builds, while still honoring an explicit
`CUDAARCHS` override.

The NSIS packaging step needs a system NSIS 3.13 plus the Tauri
`nsis_tauri_utils` plugin because the Tauri-bundled NSIS binary requires the
legacy 32-bit `MSVCP60.dll`. The generated installer was produced manually with
the system NSIS after Tauri's bundled `makensis` failed. Live Windows capture ->
recognition -> insertion, installer execution and editor compatibility remain
unverified. In particular, elevated apps may reject input from a normal process.
No security protections are weakened to bypass that boundary.

Primary references:

- https://v2.tauri.app/distribute/windows-installer/
- https://v2.tauri.app/start/prerequisites/
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput

## Mac package

`artifacts/clide_2.1.3_aarch64.dmg` is Developer ID signed, Apple-notarized,
stapled and Gatekeeper-verified. Version 2.1.3 was checked inside the mounted
DMG. This is the Apple Silicon build with the precompiled Metal library. No
installed application was replaced.

The previous `artifacts/clide_2.1.2_aarch64.dmg` is the older signed build.

## Windows package

`artifacts/clide_2.1.2_x64-setup.exe` is an unsigned, experimental x64 NSIS
installer. WebView2 must already be installed. It includes the required
DirectML and Visual C++ runtime DLLs; models are downloaded only when selected.
It is the CPU-only package and does not contain the CUDA enablement described
above. It has not been executed on a native Windows desktop.

`artifacts/clide_2.1.2_cuda_x64-setup.exe` is the CUDA-enabled x64 NSIS
installer built on the T4 host. It is 471,512,593 bytes with SHA-256
`ef4c3eba739ff457505e25319d9dedcf46fb1c31fd0e6410233444e793688474`.
It includes `clide.exe`, DirectML, the ONNX Runtime CUDA provider,
`cudart64_13.dll`, `cublas64_13.dll` and `cublasLt64_13.dll`; the latter is
477 MB and accounts for most of the package size. It is unsigned and has not
been installed or executed. Models are still downloaded only when selected.

**Compatibility warning:** this artifact was compiled with `CUDAARCHS=75` and is
only suitable for Tesla T4-class GPUs. RTX 30/40/50 cards need a rebuild with
`CUDAARCHS=all-major` (the new default in the build script).

`artifacts/clide_2.1.3_cuda_x64-setup.exe` is the rebuilt installer for current
NVIDIA cards. It is 575,515,354 bytes with SHA-256
`2cc8b779473e15acdd0742aff45a5363b18fd166f3c46fca4032538fbcc30ca5`.
Its release build used `CUDAARCHS=75;86;89;120`; CMake emitted
`compute_120a/sm_120a` for the last entry. The executable contains `sm_75`,
`sm_86`, `sm_89` and `sm_120a` kernels, plus PTX for `sm_75`. That covers the
Tesla T4, RTX 20/30/40 series and RTX 50 series, including the RTX 5050.
It still requires a CUDA 13-capable NVIDIA driver (580 or newer) and WebView2,
and it is unsigned and was not executed on a Windows desktop.
