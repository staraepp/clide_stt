# Clide v2 — Agent Handoff

**Canonical repository: https://github.com/staraepp/clide_stt** (branch `main`).

Do not push to `staraepp/clide-react----official-release-builds-`; that
repository is for release binaries.

**Purpose:** if the session building Clide ends, another agent picks up here.
Read `blueprint.md` (product truth) and `AGENTS.md` (engineering rules) first —
this file only records *state of the build*, never product decisions.

**Last updated:** 2026-09-03, expansion pass — idle shader reworked, HUD shader
added, Hold/Press copy fixed, ad assets delivered, **four cloud providers and
local Whisper added (130 tests passing, clippy clean)**. Issue 1 still blocked
on the missing Groq key.

> Update this file at every milestone, not at the end of a session. The user asked
> for this explicitly. A milestone is: a decision made, a file group rewritten, a
> build passing or failing, a test run.

## REVIEW OF THE SHADER PASS (2026-09-03)

Independently re-verified, not taken from the log above: `cargo test` **92
passed / 1 ignored**, `cargo clippy --all-targets -- -D warnings` **clean**, the
packaged app launches with neither the language warning nor a schema error, and
the live dashboard renders correctly with cards and type crisp.

**Capture caveat for the next agent:** `app_screenshot` returns a *blank* window
for this app — the accelerated WebGL layer defeats that capture path. The DOM is
fine; use `screencapture -o -x -l <window_id>` instead. Do not conclude the UI is
broken from a blank `app_screenshot`.

### Accepted: the aurora adaptation

Porting the site's simplex-noise + domain-warped field into the existing
one-pass WebGL1 renderer is the right call, and deliberately dropping the
pointer flowmap, bloom, grain, and grid pass is exactly right for a utility that
idles behind other windows. The oceanic palette staying gated behind
`u_presence` preserves "blue means voice." Keep this.

### RESOLVED: the idle aperture was removed and replaced

The user confirmed the idle canvas *should* carry something — "the app feels too
empty" — so this was not a straight deletion. What replaced it:

**One field, two intensities.** The resting state is now the same simplex
aurora, rendered in cool blue-grey (`neutralMist` `#E7EFF5`-ish, `neutralDeep`
`#D9E5EE`), weighted toward the empty lower canvas by `lowerBias` and fading
back as `restingScale` drops with presence. As dictation starts it gathers into
the site's oceanic blues. There is no second object competing with the ribbon,
and "blue means voice" still holds because the resting tone is blue-*grey*, not
voice blue.

The warm constants are gone: `porcelain` and `paperShadow` were the only warm
values (R > G > B) in an otherwise entirely cool system, and on screen they read
as a tan smudge.

**Also added:** the HUD chip now runs the same shader at chip scale
(`Hud.tsx`), because it is the only thing visible while dictating into another
app. It is suppressed in the failure state, where blue would wrongly read as
"still working".

### Original finding (kept for context): the idle aperture

The fan of contour shells anchored at uv `(0.79, 0.18)` works against the
approved direction on three counts:

1. **It breaks the palette's temperature.** `paperShadow`
   `vec3(0.700, 0.690, 0.665)` and `porcelain` `vec3(1.0, 0.984, 0.954)` are
   *warm* — R > G > B — inside a system whose every other value is cool
   (paper `#F4F9FD`, ink `#0A2338`, voice `#5B9BC9`). On screen it reads as a
   tan smudge, not as paper relief.
2. **It is decoration, not atmosphere.** blueprint §19 says shaders live
   underneath the UI and lists gradients, glow and grain as removed for good.
   The log shows the first version was rejected as "too faint" and then
   strengthened — a process pushing toward *more* visible ornament, which is the
   opposite of the brief.
3. **It competes with the signature.** The approved design spends its boldness
   in one place: the ribbon, which is functional and shows dictation state. A
   second, decorative focal point in an empty corner dilutes that.

The motivation was legitimate — the idle canvas is nearly flat and the
lower-right is empty. But the answer is either to accept quiet emptiness (which
is what "clide disappears while working" implies) or to put something
*functional* there, not an ornament.

**To remove:** delete the aperture block in `shader.glsl.ts` (the `apertureUv`
through `shells`/`shellShadows` mixes) and the `porcelain` / `paperShadow`
constants. The aurora and voice-presence code are independent and stay.

### RESOLVED: prompt copy ignored dictation behaviour

`Ribbon.tsx` took a `behavior` prop and now says "Hold" or "Press" to match the
card beneath it.

## LOCAL MODELS (2026-09-03) — Whisper done, Parakeet next

**The runtime risk is gone.** Both `whisper-rs 0.16` (with the `metal` feature)
and `parakeet-rs 0.3` were proved to build on this machine in an isolated probe
crate *before* anything was wired into Clide — which is the sequencing the plan
below always called for. Do the same for any future engine.

### What shipped

- **`models/catalog.rs`** — the installable list. Whisper Base / Small /
  Large v3 Turbo, from ggerganov's `whisper.cpp` repo on Hugging Face. Users
  pick from this; they never type a path (blueprint §13).
- **`models/store.rs`** — **the filesystem is the source of truth.** A model is
  installed when its file exists at roughly the expected size, not when a row
  says so, so a hand-deleted model or a killed download resolves correctly on
  the next launch. Tested against the truncated-download case specifically.
- **`models/download.rs`** — streams to a `.partial` file and *renames on
  success*, so an interrupted transfer can never be mistaken for an installed
  model. Emits `model:progress` / `model:complete` / `model:failed`.
  Includes a hand-rolled SHA-256 (verified against published vectors and
  against `shasum -a 256`) so checksum verification adds no dependency.
  Catalogue entries currently carry `sha256: None` — Hugging Face does not
  publish stable digests for these files. Fill them in if a trustworthy source
  appears; the verification path is already wired.
- **`providers/local/mod.rs`** — `LocalWhisperProvider`. No credential, no
  network. Inference runs in `spawn_blocking` because it is CPU/GPU-bound and
  would otherwise stall the async runtime.
- **`commands/models.rs`** — `list_models`, `download_model`, `remove_model`.
  `download_model` returns immediately and reports through events; blocking on
  a 1.5 GB transfer would freeze the webview.

### The design decision worth keeping

`LocalWhisperProvider::models()` returns **only what is installed**, read from
disk each call. An engine advertising weights the user has not downloaded would
fail at the worst possible moment. This broke an existing registry test that
assumed every provider ships a fixed catalogue; the invariant was corrected
rather than the behaviour — see `every_provider_that_offers_models_defaults_to_
one_of_them` and its cloud-only counterpart.

### Parakeet — not wired yet, and the path is known

`parakeet-rs 0.3` builds here and does ASR via ONNX Runtime. To finish it:

1. Add a `Engine::Parakeet` entry to `models/catalog.rs` pointing at the ONNX
   weights (the crate's README names the expected artifacts; Parakeet ships as
   several files, which is exactly why `store.rs` gives each model its own
   directory rather than assuming one file).
2. `store.rs` currently checks a single `file_name`. Multi-file models need
   `is_installed` to check every expected artifact — extend `CatalogEntry` with
   a list rather than special-casing Parakeet.
3. Add `providers/local/parakeet.rs` implementing the same trait. It needs
   16 kHz mono f32, which `read_wav_as_mono_f32` already produces — lift that
   helper into a shared spot.

### Not built: the model manager UI

The backend is complete and the commands exist. There is no React screen yet.
It should be a **list**, not a browser (blueprint §13): name, size, speed and
quality class, and one button that switches between Download / progress bar /
Remove. `list_models` returns everything it needs including `sizeLabel`.

## PROVIDERS (2026-09-03) — done

Five cloud backends now ship: **Groq, OpenAI, Deepgram, ElevenLabs,
AssemblyAI**. All are registered in `providers/registry.rs` and appear in the
settings UI automatically, because that screen renders from capabilities rather
than from a hardcoded list. `109 passed, 1 ignored`; clippy clean.

### What was extracted rather than copied

- **`providers/http.rs`** — reading the clip with a size ceiling, `Retry-After`,
  and status-to-`ProviderError` mapping. It also parses the five *different*
  error envelopes these providers use (`{"error":{"message"}}`, `{"err_msg"}`,
  `{"detail":{"message"}}`, `{"detail"}`, `{"error"}`, `{"message"}`) so a
  failure always reaches the user as a sentence instead of raw JSON.
- **`providers/openai_compatible.rs`** — the OpenAI `/audio/transcriptions` wire
  format. Groq deliberately mirrors it, so Groq was refactored onto this shared
  path rather than keeping a private copy. Groq's own tests guarded that move.

### Where each provider actually differs

| | auth | body | notes |
|---|---|---|---|
| Groq / OpenAI | bearer | multipart | identical wire format |
| Deepgram | `Authorization: Token` | raw audio | options are query params |
| ElevenLabs | `xi-api-key` | multipart | `model_id`, `language_code` |
| AssemblyAI | `authorization` | upload → job → **poll** | async; suits imports better than dictation |

AssemblyAI is the odd one: it is asynchronous, so `transcribe` polls every
400 ms with a 120 s ceiling. That is a reason to offer it, not to hide it — but
it will feel slower than the others for live dictation.

### Capabilities are declared honestly

Deepgram streams in reality, but its adapter reports `streaming: false` because
Clide has no streaming path yet. Capabilities describe *what this adapter
implements*, not what the vendor's API can do. Keep it that way — the pipeline
trusts these flags.

## AD ASSETS (2026-09-03)

The user is making an advertisement. Delivered `clide-press-kit.html` — the
approved design system rendered as a screenshot source: wordmark + palette
lockup, the dashboard in a *ready* state with usable sample copy, and all four
HUD states. Kept as HTML rather than PNGs so any region can be captured at any
scale.

A live screenshot is not usable for advertising while the app reports "Setup
needed" and the transcript history reads "This is a simple test." That is
another reason to get the Groq key restored.

**Capture note:** `screencapture -o -x -l <window_id>` is the reliable path;
get the id from Quartz `CGWindowListCopyWindowInfo`.

## CURRENT SESSION — open-issue pass (2026-09-03)

### Milestone: issue 1 verification attempted, blocked on runtime prerequisites

The canonical `main` checkout is active and the existing bundled app was launched
with debug logging. It registered the configured `Cmd+Period` shortcut, but its
own readiness UI currently reports:

- Groq: API key needed
- Microphone: not granted
- Accessibility: not granted

Therefore the required native TextEdit check has **not** run yet. Do not treat
this as evidence for or against the `focusable: false` fix. Issue 1 remains open
until those user-owned prerequisites are restored and a real hold-to-talk run is
performed with focus in TextEdit.

Follow-up during the shader pass: the packaged app now reports both Microphone
and Accessibility as granted. The Groq credential is the only remaining
readiness blocker before the native TextEdit verification can run.

### Milestone: issue 2 fix implemented; first test invocation blocked by PATH

`settings::load` now deserializes `dictation.language` as the same
`Option<String>` shape that `settings::save` persists, so valid JSON `null` is
no longer treated as a corrupt string. A regression test covers automatic
language (`None`) save/reload. The first `cargo fmt --check` / focused-test
invocation did not run because this shell could not resolve `cargo`; verification
is pending a rerun with the installed toolchain's absolute path.

The absolute-path rerun reached Cargo, but repository-wide `cargo fmt --check`
reported pre-existing formatting drift across many unrelated Rust files and the
chained focused test did not start. No formatting rewrite was applied. Verify the
edited file separately and run the focused test as an independent command.

### Milestone: issue 2 focused regression suite passes

`cargo test --manifest-path src-tauri/Cargo.toml settings::` passes all 5
settings tests, including `automatic_language_round_trips_as_none`. The fix is
functionally verified at the unit boundary. Repository-wide rustfmt drift remains
outside this issue; the touched settings module is checked independently.

`rustfmt --edition 2021 --check src-tauri/src/settings/mod.rs` also passes after
normalizing one pre-existing assertion layout in that same module.

### Milestone: issue 2 debug build passes

`cargo build --manifest-path src-tauri/Cargo.toml` completed successfully with
the language deserialization fix. Runtime launch-log verification is next.

### Milestone: issue 2 closed by runtime verification

The rebuilt debug executable launched against the existing database row
`dictation.language = null`, registered `Cmd+Period`, and emitted no unreadable-
setting warning. Issue 2 is fixed at both the unit and live startup boundaries.

### Milestone: issue 3 source-of-truth decision

Code inspection confirmed that every status/read path already asks the
credential file directly, while SQLite's `provider_configs.credential_configured`
is only written as a mirror. The mirror will be removed rather than promoted:
the credential file remains the single source of credential truth, and
`provider_configs` will retain only per-provider model selection. A versioned
SQLite migration will preserve existing model rows while dropping the staleable
flag.

### Milestone: issue 3 storage rewrite implemented

The provider store no longer models or writes `credential_configured`.
Credential save/remove commands now mutate only the authoritative credential
store. Schema version 2 rebuilds `provider_configs` with `provider_id`,
`model_id`, and `updated_at`, copying existing model selections before dropping
the mirrored flag. Provider tests now cover model create/update, and a migration
test covers model preservation, column removal, and `user_version = 2`.

### Milestone: issue 3 focused tests pass

`database::providers` passes 2/2 tests and
`database::schema::v2_migration_preserves_models_and_drops_credential_mirror`
passes. The migration preserves the selected model, removes the credential
column, and advances the database version. A pre-existing rustfmt line-wrap in
the touched schema file was normalized; touched-file formatting is rerun next.

All four touched Rust files pass targeted `rustfmt --check`.

### Milestone: complete Rust verification passes

The full Rust suite passes: **92 passed, 1 ignored** (the intentionally manual
focused-app insertion test). `cargo clippy --all-targets -- -D warnings` is also
clean. No local-model work was started.

### Milestone: issue 3 debug build passes; first backup check fails safely

The post-change debug build completed. Before migrating the live database, the
first attempt to create/read a SQLite-consistent temporary backup failed with
SQLite error 14. The rebuilt app was not launched, so the live database remains
at schema version 1 and was not modified by this attempt. Backup creation must
succeed before live migration verification proceeds.

Follow-up inspection confirmed the backup was created successfully at
`/tmp/clide-schema-v2.qw48Wr/clide.sqlite3`; the error came from the CLI's
`-readonly` open mode on this WAL-mode copy. A normal SQLite open reports
`integrity_check = ok`, schema version 1, and all 3 transcripts. The safety gate
is satisfied; the backup is retained through live migration verification.

### Milestone: issue 3 closed by live migration verification

Launching the rebuilt app migrated the existing database from version 1 to 2.
Post-launch `PRAGMA integrity_check` returns `ok`; all 3 transcript rows remain;
and `provider_configs` now contains only `provider_id`, `model_id`, and
`updated_at`. The credential mirror is gone from both code and live storage.

### Milestone: first debug app-bundle build invocation fails on shell PATH

`npm run app:build -- --debug` reached Tauri but could not spawn
`cargo metadata` because Cargo is absent from this shell's PATH. No bundle was
replaced. Rerun after loading the installed Rust environment; this is the same
tool-discovery issue seen during the first focused test invocation.

### Milestone: updated debug bundles pass

After loading `/Users/brenomenezes/.cargo/env`,
`npm run app:build -- --debug` completed successfully. TypeScript type-checking
and the Vite production build passed, Rust compiled, and Tauri produced both
`src-tauri/target/debug/bundle/macos/clide.app` and the arm64 debug DMG.

### Milestone: packaged-app launch passes

The rebuilt bundled `clide.app` launched against the migrated live database,
registered `Cmd+Period`, and emitted neither the old language warning nor a
schema error. Issue 1 is still blocked by the readiness prerequisites recorded
above; no TextEdit result is claimed.

### Milestone: final scope audit

The working tree changes are limited to the issue-2 settings fix/test, the
issue-3 provider-store migration/tests, and this handoff. `git diff --check`
passes. The dictation pipeline was not re-architected, no local-model work was
started, and the packaged app is left open on `C27F398` (the 60 Hz second
display) for the remaining user-owned readiness steps.

## CURRENT SESSION — website shader adaptation (2026-09-03)

### Milestone: inactive-state follow-up implemented

The shader now gives inactive dictation a distinct non-blue visual: an
asymmetric fan of pearlescent contour shells, designed as a listening aperture
pressed into paper. It breathes slowly at Normal/High, stays static in Reduced,
and recedes as `u_presence` rises so the existing blue voice field takes over.
This preserves the blueprint's “blue means voice” rule while making the idle
canvas deliberately visible instead of nearly flat.

### Milestone: inactive-state frontend build passes

`npm run build` passes TypeScript checking and the Vite production build with
the new idle aperture. `git diff --check` is clean. Shader compile/link and
visual comparison of inactive versus active remain pending.

### Milestone: inactive/active shader comparison passes

A disposable two-panel Vite harness imported the production shader and rendered
Inactive/Normal beside Active/High with mic energy. Both WebGL1 programs compiled
and linked. The first idle treatment was rejected as too faint; the final pass
uses paired warm-gray shadow and porcelain highlight ridges, making the
asymmetric fan clearly visible while remaining non-blue. The active panel stays
cyan/blue and contains no idle fan. All disposable preview files were removed.

### Milestone: inactive-state app and DMG bundles pass

`npm run app:build -- --debug` completed with the Rust environment loaded.
TypeScript, Vite, Rust compilation, `clide.app`, and the arm64 debug DMG all
pass with the final idle-state shader. Packaged-app inspection on `C27F398`
remains pending.

### Milestone: first packaged idle layout rejected and corrected

The packaged dashboard was inspected on `C27F398`. The embossed fan rendered,
but its upper-right focal point sat mostly behind the opaque bento cards. The
aperture is now anchored at normalized y `0.18`, placing it in the dashboard's
open lower-right canvas. A replacement bundle and second packaged visual check
are pending; do not treat the first bundle as final.

### Milestone: inactive-state shader passes packaged visual QA

The replacement bundle was launched and moved to `C27F398` (60 Hz). The idle
aperture is now clearly visible in the open lower-right portion of the real
dashboard: warm-gray recessed arcs with porcelain highlights, without blue or
text interference. Cards, controls, and the large empty reading area remain
crisp. The final app and DMG bundles pass, and the packaged app is left open on
the second display. The shader follow-up is complete.

### Milestone: source inspection and rendering decision

The shader source in `staraepp/Clide-website` commit
`6514b1a9300e2845b62a2c4949e09d11c6f49b3d` was inspected directly. Its hero
uses a two-pass WebGL2 pointer flowmap, simplex-noise fluid field, bloom, grain,
and a separate elastic grid.

Clide will adapt the recognizable simplex/domain-warped aurora and oceanic
palette into its existing one-pass fullscreen-triangle renderer. The website's
pointer flowmap, bloom, grain, and grid are deliberately excluded: they add
continuous utility-app GPU cost and would weaken the sharp light UI. The field
will remain pearl-neutral at rest, gather into voice blue only while dictation
is active, react to mic energy only on High, and retain the existing Reduced /
Normal / High caps plus blur/visibility pausing.

### Milestone: adapted shader implemented

`src/shaders/shader.glsl.ts` now uses the website shader's 3D simplex-noise
family and two-stage domain warping to form fluid contour ribbons. It remains a
single WebGL1 pass with no textures or framebuffers. Neutral pearl/mist colors
render at rest; the website's `#E4F3FB`, `#8BD7F2`, and `#2F9CD4` palette mixes
in only through eased dictation presence, with High-mode mic energy widening
and brightening the ribbons. `ShaderBackground`'s existing frame caps, Reduce
Motion override, DPR cap, and blur/visibility pausing are unchanged.

### Milestone: standalone WebGL compile/render QA passes

A disposable Vite page imported the real shader module and rendered 960x720
idle and active frames in Chromium WebGL1. The program compiled and linked;
idle remained pearl-neutral and the active High / energy 0.65 frame produced
the intended fluid cyan/blue ribbons. The only console error was the disposable
page's missing favicon. The preview harness is removed before packaging.

### Milestone: production frontend build passes

`npm run build` passes TypeScript checking and the Vite production build with
the adapted shader bundled into the main entry. `git diff --check` is clean.

### Milestone: updated app and DMG bundles pass

`npm run app:build -- --debug` completed with the Rust toolchain environment
loaded. Tauri produced the updated `clide.app` and arm64 debug DMG with the new
shader bundled.

### Milestone: packaged dashboard visual QA passes on the second display

The rebuilt app launched and was moved to `C27F398` (60 Hz) before visual
inspection. The live dashboard keeps cards, typography, and controls crisp over
the subtle pearl resting field; no clipping, overlap, or blank WebGL canvas was
observed. The stronger voice-blue active state remains covered by the standalone
WebGL render check because starting microphone capture without an explicit test
utterance was avoided.

### Milestone: shader scope audit passes

`git diff --check` passes after removing the disposable Playwright preview logs.
The final shader change is isolated to `src/shaders/shader.glsl.ts`; the existing
React renderer, dictation pipeline, and local-model plan were not changed. The
rebuilt packaged app is left open on `C27F398` for user inspection.

---

## ACTIVE WORK — light retheme (approved 2026-09-03)

The dark blue-black v1 was rejected: *"giving 2010 trying to be edgy."* The user
asked for light, SaaS, low-contrast, harmonious (Notion / Whispr Flow as touchstones).

**The identity is not ours to invent — it already exists.** The marketing site
served at `localhost:3000` (a Next.js project elsewhere on this machine) defines
it. Read from the live page:

| | |
|---|---|
| Ground | `#F4F9FD` |
| Ink | `#0A2338` (deep navy — no black anywhere) |
| Display | **Montserrat 500** (medium, not bold) |
| Body | **DM Sans** |
| Wordmark | lowercase `clide` + the 5-bar waveform mark |
| Shortcut | **⌥ .** — *the site says this; the build defaulted to ⌥ Space* |

Approved mockup: https://claude.ai/code/artifact/2850ebb1-aff7-4cc4-91b1-4d80822c3cee

### The design rules that came out of it

1. **Blue means voice.** `--color-voice` appears only where clide is hearing or
   handling speech: the waveform, the listening state, the caret. Never on
   buttons, links, or headings. The primary button is ink. This is what keeps the
   contrast low — a blue appearing somewhere new is a bug, not a decoration.
2. **The ribbon is the signature.** One element in the hero card that morphs
   prompt -> live meter -> typed transcript -> landed. Everything around it stays
   quiet so it carries the screen.
3. **Removed for good:** gradients, glow, grain, 22px radii, every shadow but one,
   the dark shader. That was the "2010 edge."

### Retheme checklist

- [x] Self-hosted Montserrat + DM Sans + DM Mono as woff2 in `src/styles/fonts/`
      (88 KB total, latin subset). Bundled by Vite; the app stays offline.
- [x] `src/styles/theme.css` rewritten. Tokens are now `paper / card / sunken /
      ink / ink-2 / ink-3 / line / line-2 / voice / voice-deep / voice-tint`
      plus `ok / warn / stop`. Tailwind v4 `@theme` generates the utilities, so
      components use `text-ink` and `border-line`, not the old
      `text-[--color-ice-50]` arbitrary syntax.
- [x] Every component moved off the old tokens (verified by grep — zero
      `ice`/`abyss`/`hairline`/`grain` references remain)
- [x] `shaders/shader.glsl.ts` reskinned. New `u_presence` uniform, eased in the
      render loop, so the wash gathers while dictating and recedes after.
- [x] `DEFAULT_SHORTCUT` -> `Alt+Period`
- [x] `tauri.conf.json` — `productName` and window title are now lowercase
      `clide`, `backgroundColor` `#F4F9FD`
- [x] `blueprint.md` §19 rewritten around the light system and the colour rule
- [x] Rebuilt, launched, screenshotted — the light theme renders correctly:
      Montserrat headings, DM Sans body, `⌥ .` keycaps, pale wash background

## THE MILESTONE IS MET

The user ran real dictations on 2026-09-03 while this session was building.
From the app's own log:

```
microphone open  sample_rate=24000 channels=2 sample_format=F32
transcription complete  provider=groq model=whisper-large-v3-turbo
                        latency_ms=388 characters=23
transcript saved
accessibility insertion declined; pasting
```

Two runs, 388 ms and 345 ms end to end. Text "This is a simple test." — correct,
and Polished capitalised it and closed the sentence. Transcripts landed in
SQLite. **shortcut -> capture -> transcribe -> process -> insert -> history
works.**

## OPEN ISSUES AND RESOLUTIONS, in priority order

### 1. Insertion always falls back to clipboard paste (unverified fix in place)

Both runs logged a fallback, for two different reasons:

- `nothing is focused to type into` — no focused AX element at all. This is
  what a focus-stealing HUD looks like.
- `the focused control does not accept direct text` — expected and correct;
  the target was clide's own WKWebView textarea, and web views refuse direct
  `AXSelectedText` writes.

**Fix applied but NOT yet confirmed:** the HUD window now declares
`"focusable": false` in `tauri.conf.json`. tao overrides `canBecomeKeyWindow`
and `canBecomeMainWindow` from that flag
(`tao-0.35.3/src/platform_impl/macos/window.rs:417-432`), so no `tauri-nspanel`
dependency is needed after all.

**How to verify:** focus TextEdit (a *native* control, not a web view), hold
`⌥ .`, speak. The log should show neither fallback reason and the text should
arrive without the clipboard being touched.

**Watch for a side effect:** a non-key window may not deliver clicks to its
webview. If the HUD's error-state Retry/Copy buttons stop responding, do not
undo `focusable: false` — dictation reliability outranks HUD affordances
(AGENTS.md decision priority). Move recovery to the dashboard and tray instead.

### 2. [FIXED 2026-09-03] `dictation.language` warned on every launch

`settings::load` now reads `Option<String>` consistently with the shape written
by `settings::save`. Unit coverage passes and the rebuilt packaged app launches
against the existing `null` row without the warning.

### 3. [FIXED 2026-09-03] `provider_configs` credential state could drift

Schema v2 removes `credential_configured` from SQLite. The dedicated credential
file is now the only source of credential truth; `provider_configs` stores only
per-provider model selection and its update timestamp.

## DO NOT REPEAT

This session ran `rm -rf ~/Library/Application Support/com.staraep.clide` to get
a clean onboarding run, while the user was actively testing. It destroyed their
history and settings. The Keychain entry survived (different store), which is
why transcription kept working. **Do not wipe app data without asking.**

### New in this pass

- **`src/dashboard/Ribbon.tsx`** — the signature element, extracted so the hero
  card stays readable. One surface that changes what it holds (prompt -> live
  meter -> transcript -> failure reason) rather than four swapped panels.
- **Wordmark is lowercase** across UI copy, tray menu, and `productName`. Note
  this renames the bundle to `clide.app`, which resets TCC — permissions must be
  granted again after this build.
- `lib/format.ts` gained `clockTime()` and punctuation-key glyphs, so `⌥ .`
  renders as a full stop rather than the word "Period".

### Also worth telling the user

The site's providers section says *"the providers layer is a single Swift file."*
The app is Rust/Tauri. The site copy is wrong, not the app.

## Status

| Area | State |
|---|---|
| Scaffold (Vite, Tailwind v4, Tauri 2, icons, two window entries) | done |
| Rust: state machine, audio, providers, processing | done, unit-tested |
| Rust: insertion (AX + clipboard), keychain, permissions | done |
| Rust: SQLite + FTS5, settings, shortcuts, HUD, tray, commands | done |
| `cargo test` | **92 passing, 1 ignored** |
| `cargo clippy --all-targets` | **clean** |
| Frontend: design system, shader, HUD, dashboard, history, settings, onboarding | done |
| `tsc --noEmit` + `vite build` | **clean** |
| Bundled `.app` builds, launches, registers `Cmd+Period`, opens schema v2 | **verified** |
| Live DB: schema, FTS5 match, index follows deletes | **verified by direct query** |
| Native TextEdit AX run without clipboard fallback | **BLOCKED: Groq key required** |

### Why the end-to-end run is still open

Microphone and Accessibility are now granted. The only remaining prerequisite
is a Groq API key in the current credential store.

The user has now authorized Computer Use, but only on the second 60 Hz display:
`C27F398`. The main display is `ASUS VG277Q1A` at 165 Hz and must not be used for
verification. The packaged Clide window is currently back on `C27F398`.

## Next steps, in order

1. Enter the Groq key in the bundled app.
2. Then the blueprint's actual milestone: focus TextEdit on the second, 60 Hz
   monitor, hold `Cmd+Period`,
   speak, confirm text appears **in TextEdit** and a row appears in History.
3. Watch specifically for:
   - **HUD focus stealing.** It is created with `focused: false` and `show()` is
     never paired with `set_focus()`. If macOS still pulls focus off the text
     field, the fix is `tauri-nspanel` (a genuinely non-activating panel) — that
     is the known escape hatch, not more `set_ignore_cursor_events` fiddling.
   - **AX insertion into the target.** `kAXSelectedTextAttribute` works in
     native text controls; web views and canvas editors will fall through to the
     clipboard path. Both are correct outcomes — check the HUD says which.
4. Only once that is reliable: polish, then v0.2 scope from the blueprint.

## Decisions already made (do not re-litigate)

- **Audio format:** 16 kHz mono s16 WAV. Groq accepts WAV directly, so there is
  no transcode step. Box-filter decimation in `audio/resample.rs`.
- **Audio buffering:** accumulated in memory, written to WAV on stop. Dictation
  clips are short; keeps file I/O out of the realtime callback. Hard cap 10 min.
- **`cpal::Stream` is `!Send`** on macOS → it lives on a dedicated worker thread
  (`audio/recorder.rs`), driven by a command channel.
- **Temp audio** is owned by `RecordedClip`, which deletes the file on `Drop`.
  120 s recovery window for explicit retry; orphans swept at startup.
- **Retry semantics:** `retryable` in `TranscriptionFailed` means *the audio is
  still on disk*, not that the error was transient. Clide never auto-retries and
  never silently switches provider.
- **Transcript is persisted to SQLite *before* insertion** so a failed insertion
  can never lose a good transcript.
- **Two Vite entries:** `index.html` (dashboard) + `hud.html` (HUD), so the HUD
  stays cheap.
- **Default shortcut:** `Alt+Period`, behaviour configurable Hold vs Toggle,
  one shortcut only.
- **Rewrite mode** is declared in the enum but returns `ModeUnavailable` — never
  silently falls back to Polished.
- **`NSPasteboard` is not thread-safe.** Its internal type cache corrupts under
  concurrent access and crashes inside AppKit (found via a SIGSEGV in the test
  suite). All pasteboard access goes through `clipboard::access()`, which holds
  a process-wide lock; `insert_via_paste` holds it across the whole
  borrow-paste-restore sequence so a concurrent Copy cannot interleave.
  There is a regression test (`concurrent_access_is_serialised`).
- **Mic level reaches the UI as a ref, not React state** (`useMicLevel`), and
  `Waveform` reads it in its own rAF loop. At ~30 updates/s, state would
  re-render the dashboard on every audio frame.
- **Shader throttling** is in `ShaderBackground`: fps capped per intensity
  (reduced = one static frame, normal = 30, high = 60), stopped entirely on
  blur/hidden, and system Reduce Motion overrides the app setting.
- **`DictationState`'s JSON shape is a contract** with the UI, pinned by the
  `wire_format` test in `dictation/machine.rs`.

## Environment / gotchas

- Repo path contains a space and an unbalanced `(`. Cargo verified fine; if
  `.app` bundling or codesign chokes, relocate the repo rather than fighting it.
- macOS 26.6.2, arm64, Rust 1.94, Node 26, Xcode at `/Applications/Xcode.app`.
- Ad-hoc-signed dev builds can reset TCC identity → macOS may re-prompt for
  microphone and Accessibility. Credentials no longer use the Keychain; see the
  explicit deviation below.
- **Testing display:** use `C27F398` (60 Hz), the user's second monitor, for all
  on-screen verification. `ASUS VG277Q1A` is the 165 Hz main display; do not
  screenshot or interact with it.
- Groq API key is the user's BYOK credential. The agent does not have one; the
  Groq leg of the pipeline can only be validated after the user enters it.

## Where things live

```text
src-tauri/src/
  dictation/machine.rs   the state machine — every state change goes through it
  dictation/pipeline.rs  the transaction, end to end. Start reading here.
  audio/recorder.rs      cpal on a worker thread; 16 kHz mono WAV
  insertion/mod.rs       AX first, clipboard paste second, clipboard-only last
  providers/traits.rs    the adapter contract new backends implement
  providers/groq/        the only backend in this build
src/
  lib/commands.ts        every IPC call, in one file — the contract with Rust
  lib/types.ts           mirrors the serde shapes; DictationState is a union
  dictation/labels.ts    state → words + colour, shared by HUD/dashboard/tray
  shaders/               the ambient background and its throttling rules
```

## Commands

```bash
npm install                     # frontend deps
cargo test --manifest-path src-tauri/Cargo.toml
npm run app                     # tauri dev (fast loop, weak TCC identity)
npm run app:build -- --debug    # bundled .app — use this for permission testing
```


---

# END-OF-SESSION STATE (2026-09-03)

## Shipped this session

1. **Light retheme**, matched to the marketing site. Built, launched, verified
   on screen. See "ACTIVE WORK" above for the tokens and the colour rule.
2. **Credentials moved off the macOS Keychain** — see below.
3. **HUD `focusable: false`** — applied, still unverified against a native app.

## Credentials: why this deviates from the blueprint

`blueprint.md` §14 and `AGENTS.md` both say API keys go in the macOS Keychain.
**They no longer do.** The user instructed this explicitly, and AGENTS.md allows
a blueprint deviation on an explicit instruction.

**Their reason was real.** An ad-hoc-signed build gets a new code identity on
every rebuild, so macOS sees a different application asking for the same
Keychain item and re-prompts for authorisation on every launch. That made the
app unusable during development.

**What replaced it:** `src-tauri/src/credentials/mod.rs` — a JSON file at
`~/Library/Application Support/com.staraep.clide/credentials.json`, mode `0600`
inside a `0700` directory. `security-framework` was dropped from `Cargo.toml`.

**What this costs, stated plainly:** the key is now **plaintext on disk**,
protected only by file permissions. Any process running as this user can read
it. That is strictly weaker than the Keychain.

**What is unchanged:** the key still never reaches SQLite, frontend state, logs,
or any error message. The UI still only learns `configured: true`. There is a
test asserting the file mode is `0600` and tests asserting no error variant can
carry a secret.

**How to undo it properly:** ship a build signed with a stable Developer ID
certificate. The identity stops changing between builds, the repeated prompts
disappear, and `credentials/mod.rs` can go back to `security-framework` generic
passwords behind the exact same public interface (`store` / `read` / `delete` /
`is_configured`). Nothing else in the app would change.

**Migration note:** any key the user stored in the *old* Keychain build is still
in their login Keychain and is no longer read. They need to paste the key once
more in Settings -> Transcription. There is no automatic migration, deliberately
— reading the old item would have triggered exactly the prompt this change
removes.

## ACTIVE WORK — local models (Parakeet, local Whisper)

### 2026-09-03 milestone: runtime proof started outside Clide

The user explicitly requested local models, so the earlier “do not start unless
asked” gate is cleared. The first candidate is Parakeet TDT 0.6B v3 int8 ONNX
(~670 MB of model artifacts) through `screenpipe/audiopipe` commit
`b04ec0d7fee7e3163d76fdcff24fb758dec7c729`. This is a Rust-only path with no
Python runtime. A disposable `/tmp` checkout compiled the library, but its
example failed because upstream renamed the crate to `audiopipe` without
updating four `stt::` references. Those references are patched only in `/tmp`;
no Clide dependency or source has been changed. The real transcription proof is
being rerun against whisper.cpp's 16 kHz mono JFK sample.

The architecture is already ready for it. `TranscriptionProvider`
(`providers/traits.rs`) has `local` in its `Capabilities`, and the pipeline
branches on capabilities rather than provider ids, so a local engine plugs in
without the dictation code changing.

**Implementation plan, in order:**

1. **Pick the runtime first, and prove it outside the app.** For Parakeet on
   Apple Silicon the realistic options are an ONNX Runtime build with CoreML
   execution, or `candle` with a converted checkpoint. Write a throwaway binary
   that transcribes a fixed 16 kHz mono WAV and prints text. Do not touch clide
   until that binary works — this step is where the real risk is, and it is
   entirely independent of the app.
2. **Add `providers/local/mod.rs`** implementing `TranscriptionProvider` with
   `capabilities().local = true` and `CredentialRequirement::None`. The audio
   clip handed to it is already 16 kHz mono WAV, which is what these models
   want, so no conversion is needed.
3. **Model storage.** `~/Library/Application Support/com.staraep.clide/models/`,
   one directory per model id. Add a `local_models` table (the blueprint's §26
   list already names it) recording id, version, size, installed-at.
4. **Download manager.** A `models/` module in Rust: fetch with progress events
   (`model:progress`, `model:complete`, `model:failed` — mirror the existing
   event naming), verify a checksum, unpack, record in SQLite. The UI for it is
   a list, not a browser — blueprint §13 is explicit that users must never be
   asked for a file path.
5. **Only then** surface it in the provider UI. `ProviderSettings.tsx` already
   renders from capabilities, so a local provider will appear with a "Local"
   chip and no API-key field automatically.

Do not start at step 5.

## Known issues / resolutions from this pass

1. **HUD focus fix unverified.** Focus TextEdit — a *native* control, not a web
   view — hold the shortcut, speak. The log should show no
   "accessibility insertion declined" line at all. Watch for the HUD's
   error-state buttons going unclickable as a side effect; if they do, move
   recovery to the dashboard rather than reverting `focusable: false`.
2. **RESOLVED: `dictation.language` launch warning.** The load/save shapes now
   match; unit and packaged-launch verification pass.
3. **RESOLVED: `provider_configs` credential drift.** Schema v2 removed the
   mirrored flag and live migration preserved all transcript rows.

## Test status at end of session

```
cargo test    91 passed, 1 ignored     (the ignored one types into the focused app)
tsc --noEmit  clean
vite build    clean
cargo clippy  clean as of the retheme commit
```

The user changed their shortcut to `Cmd+Period` at runtime; the default in code
is `Alt+Period`, matching the marketing site.
