# Changelog

## 2.1.3 - 2026-11-27

### Changed
- macOS Whisper now ships a precompiled Metal shader library instead of
  compiling the shader source on first launch. On an M2 Pro this cut the first
  model load from about 9.5 seconds to about 1 second; later dictation was
  already Metal-accelerated.

## 2.1.1 - 2026-10-06

### Added
- Dictionary (Settings -> Dictionary): add words and phrases to spell your way,
  and Clide learns every word you dictate. Your words respell matching text and
  are hinted to Whisper; learned words are a record you can keep, remove or
  clear. Learning can be switched off.
- Settings -> Music and other audio: lowering the Mac's volume while you record
  is now a switch.

## 2.1.0 - 2026-10-05

### Added
- Local API: an optional HTTP API on `127.0.0.1`, off by default, with a bearer
  token on every request (Settings -> Local API).
  - `GET /v1/health`, `GET /v1/models`
  - `POST /v1/audio/transcriptions` (wav, m4a, mp3, flac, caf, aiff)
  - `GET /v1/events`, a server-sent event stream: `state`, `level`, `partial`,
    `transcript`, `error`
- Settings: port, token (reveal, copy, regenerate), allowed web origins, and
  separate switches for the transcription and event endpoints.

### Changed
- Spoken-correction, name and Rewrite handling moved into one shared step used
  by both dictation and the API. Dictation behaves as before.

## 2.0.0

Local-only rewrite: Apple Speech, Whisper and Parakeet engines, voice
backtracking, Rewrite with Apple Intelligence, live typing, signed in-app
updates.
