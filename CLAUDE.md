# CallOut — developer rules

Low-latency voice chat for gamers. Rust core, Tauri 2 + Svelte 5 UI,
our own SFU server (QUIC). MVP is Windows only. Full architecture: `docs/ARCHITECTURE.md`.

## Session workflow

- **When starting**, read `docs/STATUS.md`: where we left off, what is next.
- **When finishing**, update the "Last session", "Next" and "Pending manual tests" sections in
  `docs/STATUS.md` (add anything that only a person or a real network can verify); when a phase
  is completed, also tick it in the "Phase status" list below.
- Private details such as the server IP, invite link and SSH key path live in `CLAUDE.local.md`
  (not in git). Never write them into `docs/`, `README.md` or commit messages.
- Write in English: code comments, docs, logs, error strings and commit messages are English.
  User-facing UI text is localized via the i18n dictionaries in ui/src (English is the source language).
  Talk to the project owner in Turkish.

## Layout

```
crates/callout-proto   protocol types (shared by client + server)
crates/callout-audio   cpal capture/playback, Opus, jitter, mixer
crates/callout-net     quinn QUIC: control stream + audio datagrams
crates/callout-core    Engine: Command/Event engine (audio thread + network tasks); examples/talk.rs headless client
crates/callout-server  SFU + rooms + identity, single binary
crates/callout-input   push-to-talk listener (Windows Raw Input, never swallows the key); the only crate with unsafe
apps/callout-app       Tauri shell: engine_command/get_settings/save_settings, "engine" event channel, PTT listener, tray
ui/                    Svelte 5 (runes) + TypeScript + Vite
```

## Invariant rules

1. **The cpal callback only copies into the ring buffer.** No allocation, locks,
   `println!`/`tracing`, encoding or networking. Violation = crackling.
2. **Audio data never crosses into the webview.** Tauri carries only `callout_core::Command` / `Event`.
3. **Protocol change = `callout-proto`.** Wire format is `postcard`; enum variants are
   appended at the end, and `PROTOCOL_VERSION` is bumped on incompatibility.
4. **The server does not decode Opus.** The SFU stamps the packet with `AudioPacket::stamp_sender` and forwards it.
5. **License MIT OR Apache-2.0.** No GPL/AGPL crates. When adding a new crate, check
   `cargo deny`/its license.
6. `#![forbid(unsafe_code)]` stays in library crates. If needed, unsafe code is isolated in a separate,
   small crate.
7. Error types: `thiserror` in libraries, `anyhow` in the app/server `main`.
8. Out-of-scope requests (overlay, video, screen sharing) go into the "Later" list in
   `docs/ARCHITECTURE.md`, not into code.
9. **Nothing that could be a privacy risk for users goes into the repository** (e.g. writing
   microphone audio to disk), not even as a debug tool or behind a feature flag; the shipped app never
   records (the mic test keeps its 5 s in memory only). Such diagnostics are welcome but stay local:
   patches/scripts outside the repo (location in `CLAUDE.local.md`), applied for a local build and
   reverted right after; a local pre-commit hook guards against committing them. Allowed in the app:
   the diagnostic log upload (text log, scrubbed, sent only on the user's click after the consent text,
   kept 30 days on the server).

## Commands

```
cargo test                                  # library crates (default-members)
cargo clippy --workspace --exclude callout-app --all-targets -- -D warnings
cargo run -p callout-audio --example loopback   # mic → headphones, latency measurement
cargo run -p callout-server                 # server; prints the invite link
cargo run -p callout-core --example talk -- "<invite>" Name --create   # open a room (prints the code)
cargo run -p callout-core --example talk -- "<invite>" Name --room CODE # join a room
cd ui && npm install && cd ../apps/callout-app && cargo tauri dev   # app
```

On Linux `libasound2-dev` is required; `callout-app` is built/tested in CI on Windows only.

## Phase status

- [x] Phase 0 — skeleton
- [x] Phase 1 — audio MVP: loopback, Opus, QUIC, jitter/mixer, room codes, Tauri UI (27 Oct 2026: two PCs talked through the app; server on the internet on Oracle Always Free, v0.1.2 on Releases). Gate test closed (1 Nov 2026: 4 people ~52 min, no system drops; details in `docs/GATE_TEST.md`).
- [ ] Phase 2 — audio quality (nnnoiseless, AGC, FEC, adaptive jitter)
- [ ] Phase 3 — social layer (accounts, friends, presence, text)
- [ ] Phase 4 — release (signed NSIS, updates, crash reporting)

## Decisions (26 Oct 2026)

SFU, QUIC/quinn, Opus 48 kHz 20 ms, cpal/WASAPI, Tauri+Svelte, Windows only,
device key + nickname identity (accounts in Phase 3), self-host friendly server
(self-signed certificate + fingerprint, `callout://` invite link), MIT/Apache-2.0,
target room 5–10 people (hard limit 16).

## Tauri note

`beforeDevCommand` / `beforeBuildCommand` are run by Tauri from the `apps/` folder (one level above the
tauri folder); that is why `cd ../ui` is correct and `../../ui` is wrong. `frontendDist`, on the other hand,
is resolved relative to `tauri.conf.json` (`../../ui/dist`).
