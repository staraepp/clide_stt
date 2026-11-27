# Dictation latency investigation

## Metal cold-start fix (2026-11-26)

Whisper inference was already using Metal on this M2 Pro, but the first model
load took about 11 seconds because whisper.cpp embedded the Metal *source* and
compiled every kernel at runtime. The build now ships a precompiled
`default.metallib`; on the same machine:

| Path | Before | After |
| --- | --- | --- |
| Metal library load | 11.336 s | 0.001–0.002 s |
| First 7.8 s inference after load | 12.457 s | 1.580 s |
| Later 7.8 s inference | ~440 ms | ~440–460 ms |
| App model load (`speech model loaded`) | 9.45 s | 983 ms |
| App warm-up inference | not measured | 914 ms |

The app loads the library from `Contents/Resources/default.metallib`; development
and test binaries get a copy beside the executable from `src-tauri/build.rs`.
Regenerate it with `scripts/build-metal-library.sh` when the vendored Metal
source changes.

## Follow-up optimization (2026-11-24)

Parakeet now implements the same capture-time phrase worker as Whisper, in
addition to retaining its model between requests. Empty or uncertain phrases
still trigger the complete-recording path. Final processing/insertion remains
atomic, including corrections that refer to an earlier phrase.

On the regenerated 13.292-second fixture, the follow-up release benchmark
measured Parakeet at 8,352 ms cold, 1,365 / 1,811 ms for cached full-recording
requests, and 189 ms remaining after release with capture-time recognition.
Normalized words and spoken-correction results matched the full-recording
baseline. These are new fixture measurements under current machine load, not
directly comparable to the earlier run or a live microphone-to-editor test.

Whisper's background warm-up now performs a discarded one-second inference
after loading a new model. Previously it created the model/context only,
leaving lazy GPU graph setup to the first dictation. The warm-up uses no user
audio and does not alter language settings. Its first-use latency benefit has
not yet been measured end to end.

The follow-up regression run passed 327 tests, with six manual tests ignored
and the system-volume-changing test skipped. The Windows port and its validation boundaries are documented in
`windows-port.md`.

## Changes

- Whisper recognizes completed phrases while capture continues. Final text is
  assembled before backtracking, dictionary rules, processing, history and
  insertion. No provisional text is typed into the target app.
- Phrase boundaries require at least 400 ms of near-silence and a minimum
  three-second buffer. Words are never split at an arbitrary time limit.
- One worker and a bounded, nonblocking PCM queue protect the microphone
  callback. Overflow, uncertain recognition, decode failure, a changed engine
  configuration, or 25 seconds without a safe boundary uses the original WAV
  through the existing batch path. A partial result is never used on failure.
- Parakeet holds one model/session per provider, preloads the selected model,
  reuses it across requests, and discards failed sessions. Shutdown frees it.
- Microphone startup no longer waits for volume-lowering AppleScript commands.
  A background worker checks the capture epoch before changing volume.
- Capture setup and teardown are serialized to protect quick shortcut releases.
  The lock is released before recognition. Cancelled recognition cannot deliver
  its result into a subsequent dictation.
- The resampler's final partial window reaches both the worker and the WAV.

## Local measurements

Apple M2 Pro, 16 GB, release Rust adapter, installed models. The input was a
13.292-second, 16 kHz mono synthetic speech fixture with three utterances and
700 ms silence between them. It includes “scratch that”. The same vocabulary
prompt was used for both Whisper paths. Models and inference were warmed before
the Whisper comparison; capture input was fed at real microphone speed.

| Engine / path | Measured time |
| --- | --- |
| Whisper Large V3 Turbo Q5, automatic language, full recording | 1,199 / 1,197 ms after release |
| Same model/language, recognition during capture | 297 ms remaining after release |
| Parakeet TDT 0.6B v3, first request including load | 8,244 ms |
| Same Parakeet session, second / third request | 796 / 773 ms |

Whisper's normalized word sequence and the result of applying spoken
corrections matched the batch baseline. Parakeet returned the same transcript
on all three runs. These are a local fixture measurement, not a general p95
or a measurement of the complete microphone-to-editor interaction. Compiling
the app concurrently introduced some load during the Whisper measurement.

Explicit English reduced full-recording decoding time in the same experiment.
The user's automatic-language preference and model selection remain unchanged.

Flash Attention was tested but remains disabled: with automatic language and
this build's shortened encoder context it repeatedly produced nonsense on
the same fixture. Do not enable it on the strength of speed alone.

## Reproducing the measurements

From `src-tauri`, supply an existing 16 kHz mono WAV and installed weights:

```sh
CLIDE_BENCH_MODEL=/path/to/ggml-large-v3-turbo-q5_0.bin \
CLIDE_BENCH_WAV=/path/to/phrases.wav \
cargo test --release --locked --lib bench_capture_latency -- --ignored --nocapture

CLIDE_BENCH_PARAKEET=/path/to/parakeet-tdt-0.6b-v3 \
CLIDE_BENCH_WAV=/path/to/phrases.wav \
cargo test --release --locked --lib bench_parakeet_latency -- --ignored --nocapture
```

The capture harness compares normalized words and spoken-correction output
against the full-recording baseline. It also tests Flash Attention separately
to make its rejection reproducible; production capture uses it disabled.

Fixture utterances, synthesized with macOS `say -r 170` and separated by 700 ms:

1. Please update the terminal command and check the configuration file.
2. Use the red theme. Scratch that, use the blue theme instead.
3. Keep the original words in history and run the tests before saving.

Regression coverage includes sample continuity, silence, an uninterrupted
phrase, tail draining, cross-phrase corrections, overload, cancellation and
decode failures. The normal Rust suite and frontend production build pass.
The suite's system-volume-changing test is deliberately skipped.

On this macOS/Xcode toolchain, stripped host compiler plugins failed to load
with a misaligned LINKEDIT string pool. The local app bundle is built with
`CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=none`; application stripping is
unchanged. This is a build-environment workaround, not an inference setting.

## Remaining validation

The optimization benefits phrases separated by quiet pauses. Continuous
speech, background noise or an overloaded inference worker can retain the
existing full-recording delay. Short clips still require a final decode.

The target remains less than one second from release to inserted text for
typical short dictations. This needs live microphone and focused-editor checks,
including noisy rooms, quiet voices, long uninterrupted speech, quick toggles,
first use after launch, and concurrent API/import inference. The fixture result
does not establish those gates.

The local app bundle launches and renders the Home screen. Its unsigned
development identity reports microphone access not granted and Accessibility
lost on rebuild, so live microphone-to-editor validation is blocked on those
macOS grants. No permissions were changed and the installed app was not replaced.

Transient logs now distinguish `ready_ms`, `finalize_ms`, capture `phrase_ms`,
`remaining_ms`, processing/insertion timing and total `release_ms`. No timing
fields or microphone audio are added to permanent history.
