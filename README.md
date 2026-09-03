# Clide

System-wide dictation for macOS. Hold a shortcut, speak, and the text lands in
whatever application you were typing in.

```text
shortcut → capture → transcribe → process → insert → history
```

Read [`blueprint.md`](blueprint.md) for what Clide is meant to become, and
[`AGENTS.md`](AGENTS.md) for how to work on it.

## Status: v0.1

The core dictation path is implemented. Rust owns everything native — the
microphone, the global shortcut, provider requests, Accessibility insertion,
the Keychain, and SQLite. React owns presentation only.

| | |
|---|---|
| Transcription | Groq (`whisper-large-v3-turbo`, `whisper-large-v3`), behind a provider adapter |
| Processing | Verbatim, Polished (local, deterministic). Rewrite is declared and refused. |
| Insertion | Accessibility API, falling back to clipboard paste with clipboard restore |
| History | SQLite with FTS5 full-text search. Text only — audio is never stored. |
| Credentials | macOS Keychain, bring-your-own-key |

Not in this version: file imports, local models, per-app profiles, LLM rewrite,
a customisable dashboard grid.

## Running it

```bash
npm install
npm run app:build -- --debug --bundles app
open src-tauri/target/debug/bundle/macos/Clide.app
```

Use the bundled app rather than `npm run app` (Tauri dev) for anything
involving permissions: macOS grants microphone and Accessibility access to a
bundle identity, and the dev binary does not have a stable one.

On first launch, Clide walks you through microphone access, Accessibility
access, your shortcut, and a Groq API key — then has you run one real dictation
before opening the dashboard.

## Tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run build   # tsc --noEmit + vite build
```

Two tests are ignored by default because they touch the real machine:

```bash
# writes to your login Keychain
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored keychain_round_trip

# types into whatever app is focused — focus TextEdit first
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored insertion_reaches_the_focused_app
```

## Privacy

Dictation audio is written to a temporary file, sent to the configured
provider, and deleted as soon as the transaction resolves — with a 120-second
window kept only so a failed transcription can be retried without speaking
again. History stores text, never recordings. API keys live in the Keychain and
are never written to the database, settings, or logs.
