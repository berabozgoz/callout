# CallOut — current status

> Read this file at the start of every session; at the end of a session update the "Last session" and "Next" sections.
> Private details such as the server address and invite link are not written here but in `CLAUDE.local.md`, which is not in git.

## Where we left off (2 Nov 2026)

- **Phase 0 and Phase 1 done** (gate test closed on 1 Nov; Phase 2 is next). Two Windows PCs talked over the desktop app on the same network; room codes,
  speaking indicator, jitter 40–60 ms, 0 loss on LAN.
- **Server is on the internet:** Oracle Always Free (Frankfurt, VM.Standard.E2.1.Micro, Ubuntu 24.04),
  installed as a systemd service via `deploy/install.sh`. Bera connected from the app: RTT 36 ms.
  Connection details are in `CLAUDE.local.md`.
- **Release v0.1.11** is on GitHub Releases (v0.1.10: adaptive jitter, casing fix; v0.1.11: mute/deafen icons); the server runs v0.1.9 (compatible; invite link unchanged): NSIS installer (currentUser), portable
  `callout-app.exe`, `callout-server.exe`, `callout-server-linux-x64`.
- **English + i18n (2 Nov, v0.1.9):** code, docs, logs, server output and commits are
  English. The UI is localized in 11 languages (`ui/src/i18n/*.ts`: en source, tr, de, es, fr, pt-BR, ru, ar
  with right-to-left layout, zh-CN, ja, ko; first drafts, native review welcome) with a language setting
  (system default or fixed, `Settings.language`). `Event::Error` carries an `ErrorKind` the UI translates;
  microphone access denied (`0x80070005`) has its own message. Tray labels come from the UI
  (`set_tray_labels`); the NSIS installer has the same 11 languages. Server log lines changed to English
  (e.g. `connection stats`, `connection closed`, `joined room`, `left room`) since the server update.
  An empty room name now gets a default in the creator's language. Fixed after v0.1.9 (commit a2b4fa5,
  released in v0.1.10): `index.html` declared `lang="tr"`, so CSS uppercase rendered "SETTİNGS".
- The home network is behind CGNAT, so hosting the server from home is not possible; Tailscale was not wanted.

## Next

1. **Phase 1 closed (1 Nov 2026).** Four-person test (`GATE_TEST.md`): 4 people for ~52 min total
   (35 min uninterrupted), no system drops/errors, latency and CPU criteria passed; the uninterrupted 60 min
   was missed on the participants' side. Added to the Phase 2 queue from the test: Opus FEC + PLC moved up (1–8%
   loss on real networks), Bluetooth warning, a clear error plus listen-only join when microphone permission
   is denied (`0x80070005`), panic hook writing to the log.
2. **Phase 2 — audio quality** (in order):
   0. **Loss resilience (started, 1 Nov):** Opus in-band FEC + PLC turned out to have been on since Phase 1
      (encoder assumes 10% loss, the jitter buffer hands the next packet as the FEC source for a lost
      frame). Measured with `tests/loss_sim.rs`: with isolated losses the lost-frame SNR goes from ~5 dB with PLC only →
      ~16 dB with FEC; with consecutive (burst) losses only 2.4 → 5.8 dB (FEC carries one frame back).
      If the loss assumption drops to 5%, libopus produces no FEC at all; 20–30% gives only +1 dB. Added
      `fec` (recovered via FEC) to the per-sender log: `lost - fec` = loss covered by PLC only (consecutive).
      The next decision comes from the field: if the burst loss rate is high, RED (append the previous packet to the
      datagram, a protocol change, ~2× bandwidth) or loss feedback from the receiver.
   1. **Noise suppression done (2 Nov, v0.1.8; tried on Bera's webcam mic):** `nnnoiseless`
      0.5 (BSD-3, `default-features = false`), `callout-audio/src/denoise.rs`; the 20 ms frame is processed as two
      10 ms halves. Runs on the audio thread before VAD; the level meter, VAD, encoder and mic test see the clean
      signal. RNNoise speech probability was added to VAD (≥0.85 opens, ≥0.5 holds while open). On/off in
      settings (on by default, `Settings.noise_suppression`); changing it re-measures the VAD floor.
      Measured: 45–52 dB suppression on realistic (pink/fan) noise; only ~2 dB on high-level white noise
      (an artificial case). Why: on Bera's Logitech C310 webcam mic the floor was −42 and
      speech peaks ~−32 dBFS (SNR ~10 dB); in the mic test 15% of the recording got through. In the field: floor
      −42 → −79 dBFS, sent share in the mic test 16% → 57%, voice "very clean and intelligible".
   2. **Adaptive jitter done (3 Nov, v0.1.10, not field-tested yet):** the target now follows the measured
      delay spread (`DelayEstimator` in `jitter.rs`: arrival time − sender timestamp, 95th percentile above the
      minimum over ~20 s; pre-roll bursts and sender clock restarts are ignored) instead of the late/underflow
      counter, which grew on every burst and only shrank after 30 quiet seconds. Second finding from the
      simulation: after a network stall >60 ms the buffer went idle and re-prefilled from the stale clump, so
      the rest of the talk spurt played 100–250 ms late (matches cocart's 140–240 ms); a resync now trims the
      backlog to the target. `tests/jitter_sim.rs` (old → new avg buffering): wired 38 → 37 ms, Wi-Fi 120 → 84 ms,
      Wi-Fi with stalls 127 → 99 ms, for ~1.5% concealed (late) frames. Field check: cocart's `stats` jitter_ms.
   3. AGC: target −20 dBFS, gentle gain.
   4. Automatic reconnect (exponential backoff, rejoin the same room).
   5. Fallback for UDP-blocked networks: a mode that carries audio packets over a reliable QUIC stream.
   6. Lossy network simulation test (5% loss, 100 ms jitter) — headless. Jitter part done
      (`tests/jitter_sim.rs`). The loss part is in
      `callout-audio/tests/loss_sim.rs`; the jitter (delay variation) part remains.
   7. UI polish: VAD threshold margin setting. (Listen-only join, Bluetooth mic warning and panic logging
      done 4 Nov.) **Mic test done (2 Nov,
      v0.1.8, tried in the app):** a "Mic test" section in settings — 5 s recording, played back the way
      others in the room would hear it (VAD/push-to-talk gate + Opus) along with the sent share; "Play test sound"
      (a chime); a speech-detection threshold line on the level meter (also on the in-room meter). When not connected
      and settings are open, the engine opens a local audio pipeline (`SetAudioPreview`); the selected device is
      applied without saving, "Cancel" reverts it. No audio goes to the room during recording.
3. **The push-to-talk shortcut was swallowing the key** (28 Oct, found in the two-person test): the shortcut was
   registered system-wide from app start, so a plain "T" could not be typed in other apps.
   Fixed: registered only while push-to-talk is on **and** in a room (`sync_ptt`). Outside a room
   (including the tray) the key is free. Permanent fix in Phase 2: a low-level keyboard/mouse listener that does not
   consume the key (WH_KEYBOARD_LL; `unsafe` in a separate crate, rule 6) → plain key and mouse button
   PTT in games. In a room the plain key is still swallowed; the UI shows a warning note.
4. **Ghost connection:** fixed (`Client::close_graceful`, 700 ms wait on tray "Quit") — not verified in the real
   app: the server log should show the close as `ApplicationClosed ... "bye"`, not `TimedOut`.
   In the first two-person test, mamitest's first session (joined the room, ~7 s later `in_room=false` on the server)
   is still unexplained: join/leave/mute logs were added to the server; the same test will be repeated with mute/deafen.
5. **Key capture in settings + F5 bug** (after v0.1.4, 31 Oct): the push-to-talk field is now a "press a
   key" capturer (`KeyboardEvent.code` → shortcut name; the shortcut is released temporarily while capturing,
   `ptt_capture`). F5 reloaded the webview and reset the UI while the engine stayed connected ("already connected"):
   on the app side a `Snapshot` + `engine_snapshot` command, the UI fills itself in on load; if the engine receives
   `Connect` while connected it re-broadcasts its state; in release builds F5/Ctrl+R/right-click are blocked. Not tried in the real
   app (not committed).
6. **v0.1.6 (31 Oct):** automatic recovery on audio device loss (with backoff; if the device keeps coming and going,
   the backoff does not reset until it has been stable for 30 s), per-sender jitter log (every 30 s,
   `sender jitter sender=… target_ms late lost underflow`), the actual QUIC error in the disconnect reason,
   the client `lost` counter no longer drops back. Not yet tried in the field by unplugging a device.
7. Known gaps: PTT in games (fullscreen) not verified; no mouse button PTT;
   `LICENSE-APACHE` full text missing; the CI "Libraries (Linux)" job once exited with 1
   (all steps pass locally, waiting for the log).

## Workflow

- **Version number:** the tag is the single source. `release.yml` writes the tag (`v0.1.5` → `0.1.5`) into
  `Cargo.toml` `[workspace.package] version` before building; the installer name
  (`CallOut_0.1.5_x64-setup.exe`) and the UI footer (`app.package_info()`) come from there.
  `tauri.conf.json` has no `version`. The workflow fails if the tag is not in `vX.Y.Z` form. Occasionally sync the
  `Cargo.toml` version in the repo with the latest tag (the version shown in local runs).
- Server update: push a tag (`git tag v0.1.x && git push origin v0.1.x`), and once Actions finishes run
  `curl -fsSL https://raw.githubusercontent.com/berabozgoz/callout/main/deploy/install.sh | sudo bash` on the server.
- After code changes: `cargo fmt && cargo clippy --workspace --exclude callout-app --all-targets -- -D warnings && cargo test`.
- If the protocol changes: `callout-proto` + `PROTOCOL_VERSION`; client and server are updated together.

## Last session

- 31 Oct–1 Nov: v0.1.5 and v0.1.6 released (key capture, F5 state, version from tag, audio device
  recovery, per-sender jitter log, QUIC disconnect reason); server updated to v0.1.6.
  The wired two-person 1-hour test passed (31 Oct); the four-person test was run and analysed; Phase 1 closed (1 Nov).
  Device recovery still not tried in the field.
- 1 Nov (start of Phase 2): reviewed the FEC/PLC path and measured it with a lossy network simulation
  (`tests/loss_sim.rs`), added `fec` to the sender jitter log; released as v0.1.7 (server unchanged, can stay on v0.1.6).
- 2 Nov: added a mic test to settings (record-and-listen, test sound, threshold line). fmt/clippy
  (including the app)/test/svelte-check clean; tried in the app, v0.1.8.
- 2 Nov (VAD clipping): in the mic test and in a report from cocart, word starts/ends and quiet
  words were being cut. Bera's noise floor −42 dBFS, threshold +6 dB. Speech simulation
  (`vad.rs` `sim`): at this floor 68% of speech frames got through. Fix: hysteresis (an open gate
  stays open down to 4 dB below the threshold; the close threshold is ≥3 dB above the floor), the floor is not
  updated during speech/hold, 60 ms pre-buffer (when the gate opens the previous 3 frames are also sent; in the mic test too),
  hold 300 → 400 ms; TALK_START now fires on the first packet actually sent. Result 91%. Remaining loss:
  whole sentences spoken below the threshold → noise suppression (lowers the floor) and a sensitivity setting.
  Tried in the field together with noise suppression and released as v0.1.8.
- 2 Nov (English switch and localization): four parallel passes translated code/docs/server to English
  (docs/DURUM.md → docs/STATUS.md, docs/KAPI_TESTI.md → docs/GATE_TEST.md); error kinds; i18n with 11 languages
  and RTL. Commits b84d9ee, 5b5860a, d8d8139. fmt/clippy (incl. app)/test/svelte-check clean; Arabic RTL and
  Japanese checked in the browser preview; tried in the Tauri app by Bera (English system language, tray,
  error messages OK); released as v0.1.9 and the server updated.
- 3 Nov: analysed cocart's log from the 1 Nov four-person test (`GATE_TEST.md`): on Wi-Fi, RTT + buffer
  ≈ 208 ms at cocart (above 150 ms), caused by downlink delay variation (late packets from every sender, ~5/min),
  not loss. Adaptive jitter (target a late-packet rate, shrink faster) moves up in the Phase 2 queue.
- 3 Nov (adaptive jitter): delay-spread estimator + stall resync, measured with a new jitter simulation
  (wired / Wi-Fi / Wi-Fi with stalls). fmt/clippy (incl. app)/test clean; released as v0.1.10, not field-tested yet.
- 3 Nov: mute/deafen emoji (🔇 looked like "sound off", 🙉 for deafen) replaced with Tabler outline SVG icons
  (MIT, `ui/src/Icon.svelte`): microphone(-off), headphones(-off) on the buttons and member flags. Released as v0.1.11.
- 4 Nov (small robustness items): the microphone is optional — if it cannot be opened (missing device,
  Windows privacy `0x80070005`) the error is shown but the pipeline runs output-only, so the user can join and
  listen (mic test reports 0%). New `Event::AudioStarted { input_rate, input_channels }`; the UI warns about
  listen-only mode and about a ≤16 kHz input (Bluetooth hands-free profile) in settings and in the room.
  Panic hook logs panics (app log file and server journal). The "connection lost" banner already exists via
  `error.connection_lost`; automatic reconnect is next. Not tried in the app, not released.

## Previous session

- 27–28 Oct: added Oracle (iptables) support to `deploy/install.sh`; set up the Oracle instance,
  added a public IP and a UDP 4433 ingress rule; the app connected to the server in the cloud.
