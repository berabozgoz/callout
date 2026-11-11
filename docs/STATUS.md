# CallOut — current status

> Read this file at the start of every session; at the end of a session update the "Last session" and "Next" sections.
> Private details such as the server address and invite link are not written here but in `CLAUDE.local.md`, which is not in git.

## Where we left off (11 Nov 2026)

- **Phases 0–1 done; Phase 2 (audio quality) mostly done:** FEC/PLC measured, noise suppression (RNNoise),
  adaptive jitter (delay-spread estimator + stall resync), VAD preroll/hysteresis/sensitivity, AGC (v0.1.14; retuned to a comfort band and
  off by default in v0.1.17 after the CS2 test). Left: the field checks below. Parts of Phase 4 are done early: in-app updates (signed, verified v0.1.12 → v0.1.13),
  panic logging; code signing (SmartScreen) is not.
- **Released:** v0.1.19 on GitHub Releases (installer, portable exe, both servers, `latest.json`). Installed
  copies from v0.1.12 on update in-app; older ones need one manual install of the latest release.
- **Server:** Oracle Always Free (Frankfurt) runs **v0.1.16** (v0.1.17–v0.1.19 are app-only, no server update needed) (persistent rooms in `/var/lib/callout/rooms.tsv`,
  owner-only room closing, log uploads into `/var/lib/callout/logs/`; updated 4 Nov 17:04 UTC, nobody was
  connected, invite link unchanged, 7 rooms loaded). Details in
  `CLAUDE.local.md`; update with `install.sh` (see Workflow).
- **macOS app (11 Nov, not released yet):** universal DMG (Apple Silicon + Intel, macOS 11+), outside the
  App Store, ad-hoc signed (no Apple Developer ID; users click "Open Anyway" once). Push-to-talk via a
  listen-only Quartz event tap (needs Input Monitoring), audio thread at QoS user-interactive, in-app
  updates through the same `latest.json`. Built in CI only (`app-macos` job, release `macos` job); never run
  on a real Mac yet: see "Pending manual tests" → macOS.
- **Unreleased on `main`:** the macOS app (above); otherwise nothing since v0.1.19 (check `git log v0.1.19..`). v0.1.19 brought **echo cancellation** (AEC3,
  on by default), the adaptive output reserve (30 ms at start, 20–80 ms, was a fixed 40 ms) and latency
  diagnostics (`audio latency`, `wait_ms`, `echo` log lines); see "Last session".
- **Everything is English** (code, docs, logs, commits); the UI is localized in 11 languages (`ui/src/i18n/`,
  English is the source; add keys to every dictionary, the `Messages` type enforces it).
- **Product behavior worth knowing:** rooms are private (no server-side list; only people with the code, or who
  joined before, can find them); rooms persist until closed by their creator's device or unused for 30 days;
  the client reconnects and rejoins on its own; the app starts in the installer's language once per new
  installer choice; the portable exe gets a "Download" link instead of the in-app updater.
- The home network is behind CGNAT, so hosting the server from home is not possible; Tailscale was not wanted.

## Pending manual tests

Things that passed automated checks but still need a person, a real Windows PC or a real network.
Tick and move the result to "Last session" (or `GATE_TEST.md`) when done; release only after the
items marked **before release** pass.

- [ ] **macOS: CI build** (before release): the `app-macos` CI job (clippy incl. the app, library tests on
      macOS) is green. Then a release dry run is the tag itself: the `macos` job must upload
      `CallOut_<v>_universal.dmg` and `CallOut_universal.app.tar.gz` (+ `.sig`), and `latest.json` must
      list `darwin-aarch64` and `darwin-x86_64` next to `windows-x86_64`.
      ✓ CI part, 11 Nov (run 43052387197 on PR #1, head 0c4a453): `app-macos` (clippy incl. the app,
      library tests) and `macOS test DMG` green on the first try; the universal DMG artifact is ~12.9 MB.
      The release part waits for the first tag with the macOS job.
- [x] **macOS: empty window** (before release): 11 Nov, the first test DMG showed only an empty dark
      window on the owner's 2014 MacBook Air (Intel, macOS 11.7.11, Safari 16.6.1): CSS loaded, the script
      never ran (log: only `CallOut started`). The UI was built for `chrome105` only and contained private
      class methods (`#m()`); the app's WKWebView evidently runs an older WebKit than the installed Safari.
      ✓ 11 Nov: with the Vite target `["chrome105", "safari14"]` (DMG dcb1227) the UI shows on that Mac.
- [ ] **macOS: install and first start** (before release; Apple Silicon Mac): download the DMG (before a
      release: the `CallOut-macOS-<sha>` artifact of the "macOS test DMG" CI job on a PR or a manual
      "Run workflow"; unzip it), drag to
      Applications, start: blocked → System Settings → Privacy & Security → "Open Anyway" opens it (if
      macOS says "damaged" instead, `xattr -cr /Applications/CallOut.app` must fix it; note which one
      happened, it goes into the release notes). Microphone prompt shows the Info.plist text; after
      "Allow" the sound test shows a level and plays back. Settings, languages, tray (menu bar) icon work;
      closing the window hides it, clicking the Dock icon brings it back, Quit in the menu bar icon quits.
- [ ] **macOS: talk with Windows** (before release): Mac and a Windows PC in one room on the Oracle server
      for ~10 min: both hear each other, no crackle; the Mac log
      (`~/Library/Logs/io.github.berabozgoz.callout/callout.log`) shows
      `audio thread priority raised boost="qos user interactive"` and sane `audio timing` / `audio latency`
      lines (CoreAudio buffer sizes are new territory); echo cancellation on MacBook speakers.
      Logs ✓ 11 Nov (2014 MacBook Air "macBera" + Windows "bera", room 9RYJFSXX, ~6 min, both logs sent):
      Mac `boost="qos user interactive"`, device 44.1 kHz stereo in/out (resampled), `loop_max_ms` 4–7,
      no stalls, callbacks 12 ms; `out_device_ms=11 in_device_ms=11`, reserve settled at 20 ms
      (`out_fill_ms` 28): ~50 ms device side, lower than Windows. RTT 39–45 ms, no loss after the join
      (6/8 packets lost right at the join on both sides), Windows saw `late=0 underflow=0` from the Mac.
      Open: (a) how it sounded (owner's ear); (b) the Mac's first 30 s had `out_underrun=119808` samples
      (~1.4 s) and `VAD calibrated floor=-100`, before joining a room: probably the microphone permission
      prompt (no input while it was up); check that a second start has no such underrun.
- [ ] **macOS: push-to-talk** (before release): turn on push-to-talk in Settings: the yellow "Input
      Monitoring" note with its button shows; the button brings the macOS prompt / System Settings with
      CallOut listed; after allowing and reopening CallOut the note is gone. In a room: holding a key
      (also a combination with Cmd/Option, shown as "Cmd"/"Option") and a mouse side button transmits only
      while held, the key still types in other apps, and nothing sticks after release (also when releasing
      inside a password field). The log has `push-to-talk listener active`.
- [ ] **macOS: in-app update** (after the first macOS release, with the next one): the update banner
      shows, "Update and restart" replaces the app in Applications and restarts it. Check whether
      microphone and Input Monitoring still work afterwards (ad-hoc signatures change with every build, so
      macOS may ask again or need CallOut toggled off/on in Input Monitoring; this decides whether an Apple
      Developer ID is worth it). Started from the DMG window instead of Applications: the banner offers
      "Download" instead.

- [x] **Automatic reconnect, room survives** (before release; since 0d98bc3): two people in a room, pull one
      PC's ethernet for ~5 s. Expected: "Reconnecting…" card within ~10 s, back in the same room after the
      cable returns; the other side sees leave + join. Log: `reconnect failed` attempts, then
      `reconnected; rejoining room`.
      ✓ 3 Nov: two cable pulls on the Oracle server, back in room JBRA435A after 2–4 s each.
- [x] **Reconnect keeps mute** (before release): same as above while muted; the others still see you muted.
      ✓ 3 Nov (Bera): the others still saw him muted after the reconnect.
- [x] **Reconnect "Stop" button** (before release): pull the cable, press Stop. Back on the connect screen; no
      reconnect when the cable returns.
      ✓ 3 Nov (Bera): Stop returned to the connect screen; no automatic reconnect afterwards.
- [ ] **Join/leave sounds** (v0.1.18): Settings → "Join sound": picking a style plays its join form,
      the play button plays join then leave; Save applies it, Cancel keeps the old one. In a room you hear
      the join sound when you join and when someone joins, the leave sound when someone leaves (also after
      their connection drops, ~10 s later); nothing when deafened or set to Off. Check the level against
      voices (peak 0.22, like the test sound) and that it plays on the chosen output device.
- [ ] **Remembered per-user volume** (v0.1.18): in a room, move a player's slider: the number next to
      it follows (e.g. 55%, highlighted when not 100%); clicking the number resets to 100%. Leave and rejoin,
      quit from the tray and reopen, and pull the cable for a reconnect: that player comes back at the set
      volume (number and what you hear). `settings.json` has `user_volumes` with only the non-100% players;
      saving the settings page does not drop them. Known limit: keyed by nickname (no stable user id until
      Phase 3), so a renamed player starts at 100% and two players with the same name share the value.
- [ ] **Audio thread priority under game load** (v0.1.18): the log shows `audio thread priority raised
      boost="mmcss pro audio"` after joining. In a game session (LoL or CS2, the 7 Nov / 4 Nov players)
      `out_underrun` and `late` should drop well below 7 Nov (Hallobadi: ~1.2 s underrun in 50 min, 56
      late), and `audio timing` should show `loop_stalls` near 0 with `loop_max_ms` well under 40. If
      stalls remain while `playback_max_ms` stays ~10, the game still outruns the thread; if the
      callbacks' gaps grow too, the whole PC is overloaded. Also: the desktop stays responsive.
- [ ] **Adaptive output reserve under game load** (next release): in a game session the log's `output reserve`
      lines show where it settles. Expected: on a calm PC `reserve_ms=20` after ~2 min (`audio latency`
      `out_fill_ms` ~24; with the old fixed 40 ms an estimated ~43, measured 33 at 30); on a loaded PC it rises (`underrun_samples` > 0) and
      stops at the lowest value that holds, with at most one click per tried value. Nobody should hear
      more crackling than with v0.1.18 (compare `out_underrun` in `audio dropouts`).
- [ ] **Latency breakdown in the field** (next release): from a game session's logs, per player: `wait_ms`
      (jitter buffer), `out_fill_ms`, `out_device_ms`, `in_device_ms`, `rtt_ms`. That tells which part of the
      delay is worth working on next (ms-precise jitter target, time stretching).
- [ ] **Echo cancellation in the field** (next release; the others must update too): Bera plays with kuzen
      and Hallobadi as in the LoL/CS2 sessions and should no longer hear himself. Their logs (Send log)
      show an `echo erl_db erle_db delay_ms` line every 30 s while something plays: a working canceller
      shows `erle_db` well above 0 when there is echo, and `delay_ms` near the `audio latency` sum
      (`out_fill_ms + out_device_ms + in_device_ms`, ~60–80 ms). Also check that nobody's voice got
      choppy while both talk, and that `audio timing` stays as before (CPU: ~1% of a core in the
      simulation). Settings → "Echo cancellation" off brings the echo back (A/B).
- [ ] **Echo cancellation on speakers** (next release): one player on laptop speakers instead of headphones;
      the others hear no echo after the first few seconds of talk (AEC3 learns in seconds when one side
      talks, ~20–30 s when both talk all the time).
- [ ] **Settings row** (next release): "Noise suppression" and "Echo cancellation" share one line (short
      labels, details in tooltips); the page still fits without scrolling in all languages with push-to-talk
      on and off (checked in the browser preview without Tauri: never taller than before).
- [ ] **Long outage** : cable out ~1 min. Reconnects within ~15 s of the cable returning.
- [x] **Listen-only join** (before release; since 53e73f4): Windows Settings → Privacy → Microphone → turn off
      desktop app access, connect. Expected: the microphone error is shown, but you can join a room and hear
      the others; "Listening only" hint in the room and in settings; the mic test reports 0%. Turn access back on.
      ✓ 3 Nov: mic access turned off while connected → pipeline rebuilt with has_input=false, listening went on.
- [x] **Sensitivity slider** (since d16c116): in Settings, moving it moves the threshold line on the meter;
      "More" picks up quiet speech in the sound test, "Less" cuts it; Cancel restores the saved value.
      ✓ 4 Nov (Bera): works as expected.
- [x] **Bluetooth mic warning** (since 53e73f4): pick an AirPods/Bluetooth headset mic. Expected: the 16 kHz
      warning in settings and in the room.
      ✓ 4 Nov (Bera, AirPods): the warning shows in settings and in the room.
- [ ] **AGC off after the update** (next release): on a PC that had v0.1.14–v0.1.16 with AGC on, after the
      update Settings shows it off; turn it on, Save, restart → stays on. A normal headset mic with AGC on:
      the log's `agc gain_db` stays near 0.
      ✓ 4 Nov (Bera, dev build, webcam mic): an older settings file (no flag) came up with AGC off and the
      flag set; AGC on + Save, tray Quit, restart → still on. Webcam with AGC on: `gain_db` +10…+11,
      `peak_out` −14…−16 dBFS, `limited_frames=0`. Not tried yet: the true → false switch-off on a real
      update (unit-tested), a normal headset mic (no other mic at hand). Running an older build after the
      new one drops the flag when it saves (it does not know the field), so the next new start switches AGC
      off once more: harmless, only seen when switching back and forth between builds.
      ✓ 4 Nov (Bera, v0.1.17 fresh install after uninstall): AGC off. The uninstaller also deleted the app
      data folder (`device.key`, logs, settings), so Bera got a new device key and lost "Close room" on
      the six rooms his old key (953462d4…) created: room ownership does not survive "delete app data".
- [ ] **CS2 test leftovers** (4 Nov, see `GATE_TEST.md`): push-to-talk in CS2 not tried; the mic test was run
      in the room but nobody checked whether the other side heard it; the 1 min cable pull was not done.
      AGC: both players turned it off because it sounded better without; kuzen found Bera too loud with it.
- [ ] **Adaptive jitter in the field** (since v0.1.10): a Wi-Fi player (cocart) on v0.1.10+ plays a session.
      Expected: `stats` jitter_ms around 80–100 ms instead of ~140 (v0.1.6), late/lost up slightly (~1.5%).
      7 Nov (LoL, v0.1.17): Hallobadi is behind a Wi-Fi hop (PC wired to a box that reaches the router over
      Wi-Fi); Bera's buffer for him averaged 64 ms (60 ms 85% of the time, max 140), loss ~0.5% (92% via
      FEC). Good, but cocart's worse Wi-Fi is still the real check.
- [x] **FEC field data** (since v0.1.7): any session with loss; compare `lost` and `fec` in the `sender jitter`
      lines. A large `lost − fec` means burst loss → consider RED.
      ✓ 4 Nov (CS2): FEC 77% of kuzen's loss. ✓ 7 Nov (LoL): FEC 443 of 484 from Hallobadi (92%, ~0.5%
      uplink loss, ~0.04% left for PLC). RED not needed.
- [x] **Audio device auto-recovery** (since v0.1.6): unplug and replug the headphones while in a room.
      Expected: "Audio device lost; reconnecting…" then audio back; log line `audio pipeline recovered`.
      ✓ 3 Nov: input lost (privacy toggle) → `audio pipeline recovered attempts=1`.
- [ ] **Language persists and tray follows it** (since v0.1.9): pick a language, Save, quit from the tray,
      reopen. The UI and the tray menu use the saved language; Cancel reverts an unsaved pick.
- [x] **Installer language selector** (since v0.1.9): run `CallOut_*_x64-setup.exe`; the NSIS language list shows
      the 11 languages and the installer uses the chosen one.
      ✓ 4 Nov (Bera): all 11 languages listed; picking Türkçe ran the installer in Turkish. The app itself
      still follows its own language setting (system language by default), not the installer's choice.
- [ ] **Mic test is private in a room** (since v0.1.8): run the mic test while in a room; the others must not
      hear the recording.
- [x] **Auto-update** (from the first updater-enabled release on): with release N installed, publish N+1. Expected:
      the banner appears (at start or within 6 h), "Update and restart" downloads, leaves the room cleanly and
      installs; CallOut reopens on N+1 with settings kept. In the portable exe the banner instead says to
      download the new exe and its "Download" button opens the Releases page.
      ✓ 4 Nov (Bera): installed v0.1.12 showed the banner after a restart, updated in-app to v0.1.13.
- [x] **Persistent rooms** (after the server update): create a room, everyone leaves, rejoin with the same code;
      the lobby's recent rooms list joins with one click; after `install.sh` re-runs (server restart) everyone
      reconnects into the same room.
      ✓ 3 Nov on a local server: rejoin by code and from the recent list; after a server restart the client was back in room KD2R8L42 within 1 s.
- [x] **Close room** (after the server update): the creator sees "Close room" (others don't); a second click
      closes it for everyone ("closed by its owner"), the code no longer works and it leaves the recent lists.
      ✓ 3 Nov on a local server: closed for everyone, rooms.tsv emptied. With the old v0.1.9 server the button disappears after a reconnect (old server keeps the creator's first session id) — expected until the server update.
- [ ] **Installer language becomes the app language** (next release): install picking Türkçe on an English
      Windows → the app and the tray open in Turkish; change the language in Settings, reopen → the change is kept.
- [ ] **AGC** (retuned, off by default since the next release; turn it on in Settings to test): (a) Bera's webcam mic: the others hear him at a comfortable level without
      raising the volume; (b) a loud player and a quiet one in the same room sound level; (c) with AGC on, fan or
      keyboard noise between words is not louder than before; (d) a mic with Windows' own AGC on does not make
      the level wobble; (e) the log shows `agc gain_db` lines (expect +5…+12 for a quiet mic; ✓ 4 Nov: +12 on
      Bera's webcam, `limited_frames=0`); (f) switching it
      in Settings takes effect in the mic test and Cancel reverts it.
      ✓ 4 Nov (Bera, release exe): (c) noise between words not louder with AGC on; (f) the switch takes effect
      in the mic test and Cancel reverts it. (a) and (b) need the release and other players; (d) needs such a mic.
      7 Nov (LoL, v0.1.17, both AGC on): (e) Bera's webcam median +0.2 dB in game (was +6…+8 in CS2),
      Hallobadi's gated quiet mic median +7.8 dB (cap +12), limiter in 20 of 98 windows.
      ✓ (a) Hallobadi: Bera (webcam, AGC on) sounded good, not too loud. (b) still needs a loud and a quiet
      player compared by ear.
- [x] **UI fixes before v0.1.14** (since this session): (a) a fresh window opens 780 px tall and centered; the
      settings page fits without scrolling with push-to-talk on and off; on a 1366×768 screen the window
      shrinks to the work area; (b) in a room, "Close room" opens a confirmation strip under the header
      (room name and code stay in place); its "Close room" closes the room, "Cancel" hides the strip;
      (c) turning push-to-talk on with no key starts the key capture; an old unpicked F9 (push-to-talk off)
      is cleared on start; with push-to-talk on and no key the room shows a warning.
      ✓ 4 Nov (Bera, release exe): all of the above as expected.
- [x] **Non-swallowing push-to-talk** (since this session): (a) bind a plain letter (e.g. V): in a room it
      transmits while held **and** the letter still types in Notepad / works in the game; (b) bind a mouse side
      button (Mouse 4/5): transmits while held, the browser does not go back; (c) Ctrl+key combination needs
      Ctrl, a plain key also works while Shift is held; (d) outside a room or with push-to-talk off nothing
      transmits; (e) Win+L while holding the key: transmission stops within ~0.25 s; (f) a game run as
      administrator: push-to-talk works only if CallOut runs as administrator too (expected).
      ✓ 4 Nov (Bera, release exe): plain key, mouse side button, combination, off outside a room, Win+L
      release, and in a game. (f) not tried (no elevated game at hand).
- [x] **Push-to-talk in a fullscreen game** (known gap since Phase 1): push-to-talk works while a
      fullscreen game has focus.
      ✓ 4 Nov (Bera): with the Raw Input listener, in a game.
- [x] **Log upload** (since this session): (a) an older server: "Send log" answers "This server can't receive
      logs" after ~20 s and the connection carries on; (b) a server with uploads: stored as
      `<data>/logs/<time>_<nick>_<device>.log`; (c) after a reconnect the "Something went wrong" strip shows
      once; (d) the stored log has no Windows user name or fingerprint.
      ✓ 4 Nov (Bera, release exe): (a) against the Oracle v0.1.12 server; (b)(c) against a local server
      (restarted to force a reconnect); (d) checked on the two stored logs: the name only in the header's
      nickname line, one path masked as `C:\\Users\\<user>\\…`.
- [x] **Log upload on the real server** (after the next release + server update): a friend sends a log; it shows
      up in `/var/lib/callout/logs/` (SSH) under their nickname.
      ✓ 4 Nov (CS2 test): kuzen sent two logs, Bera one; all stored under their nicknames.
- [x] **Push-to-talk in Apex Legends, app in the tray** (v0.1.16, alone on the Oracle server): hold/release
      pattern in the Firing Range while speaking; the server's `connection stats` `dgram_rx` for that user
      rises only during the holds (~50/s), nothing sticks after release, the key still does its in-game action.
      4 Nov 2026 22:46–22:53 (TR): server and client logs agree. Packets went out only in hold bursts
      (three ~5 s holds at 22:47–22:48, a long stretch at 22:49–22:50, one at 22:52). Between and after the
      holds the count stayed at the ~3/10 s idle baseline, so nothing stuck; 0 lost packets, RTT 38–50 ms,
      no capture overruns. The silent 22:50–22:52 gap was Bera alt-tabbing out to move the binding from T
      (Apex: voice chat) to 4 (Apex: heal); both keys kept their in-game action while talking. Apex in
      exclusive fullscreen.

## Next

**Start here next session** (pick with the owner):
- **macOS app (11 Nov, unreleased, draft PR #1 on branch `macos`):** macOS CI is green and the PR's
  "macOS test DMG" artifact is ready; next the macOS items in "Pending manual tests" on the owner's Apple Silicon Mac,
  then release. Known gaps: no Apple Developer ID (Gatekeeper "Open Anyway", permissions may reset on
  update; signing + notarization are prepared in `release.yml` comments), a denied microphone gives
  silence instead of the "microphone access denied" error (needs an AVCaptureDevice check), the icon is
  built from the 512 px PNG (no 1024 px retina size) and is a full-bleed square, not the macOS rounded
  shape; Caps Lock cannot be a push-to-talk key on macOS.
- **Jitter / latency work (started 8 Nov, released in v0.1.19):** steps 1–2 of the plan are done (latency log,
  adaptive output reserve). Next, once field logs show the breakdown: (3) an ms-precise jitter target instead
  of `ceil(spread/20)+1` frames (~10–15 ms on calm links; `tests/jitter_sim.rs` to measure), (4) time
  stretching (WSOLA, own code or an Apache/MIT library; SoundTouch is LGPL) to shrink/grow the buffer
  mid-spurt and absorb clock drift, (5) faster recovery after a delay spike (the 20 s window keeps the target
  high). Later: 10 ms Opus frames, WASAPI low-latency shared mode (IAudioClient3).
- **Echo cancellation (released in v0.1.19):** Bera heard himself as an echo from kuzen's and Hallobadi's
  headphones in the LoL/CS2 sessions. The echo is made at the listener's PC, so it only goes away once
  **they** run the new version. Field check: see Pending manual tests.
- **Log upload** is out in v0.1.16 (server updated): ask friends to press "Send log" after a session and read
  the logs over SSH in `/var/lib/callout/logs/`. That is the way to get the field data below.
- **AGC retuned and off by default (unreleased):** after the CS2 test (too loud, both players preferred it
  off) the AGC leaves a mic alone when its speech level is inside a comfort band (−30…−20 dBFS RMS) and only
  raises a quiet one to −30 / lowers a loud one to −20; it ships **off** by default, and settings files from
  v0.1.14–v0.1.16 get it switched off once (`agc_default_off_seen`). Checked locally on Bera's webcam
  (+10…+11 dB at normal speech, clean). The CS2 complaint was on that same webcam while speaking up in
  game (old AGC still +6…+8 dB there; the band should give ~0 for that level) → field check: Bera on the
  webcam with AGC on in a game with kuzen, no longer too loud.
- **Audio thread priority + timing stats (released in v0.1.18):** the engine's audio thread now joins
  the MMCSS "Pro Audio" task (new crate `callout-priority`, fallback `THREAD_PRIORITY_HIGHEST`), after two
  players' PCs starved it under game load (Hallobadi under LoL: ~1.2 s of `out_underrun` in 50 min matching
  his `late` bursts on a clean network; kuzen under CS2: one 0.6 s stall). Every 30 s the log has an
  `audio timing` line: `loop_max_ms`, `loop_stalls` (loop gaps ≥ 20 ms), `capture_max_ms`,
  `playback_max_ms`. Checked locally (talk example, local server): `boost="mmcss pro audio"`, loop max
  3 ms, callbacks 10–11 ms. Next: a game session with the same players and compare
  `out_underrun` / `late` with the 7 Nov numbers (see Pending manual tests).
- Field data from friends on v0.1.16: cocart's Wi-Fi jitter (`stats` jitter_ms, expect ~80–100 ms vs ~140 before),
  FEC (`lost` vs `fec` in `sender jitter`), persistent rooms / close room on the real server.
- **AGC** is released in v0.1.14 (checked on Bera's webcam mic). Field: a loud and a quiet player in one room
  (AGC (a), (b) in "Pending manual tests"); then decide whether Phase 2 can be closed. Open: with noise
  suppression off the VAD barely opens on that mic (threshold above speech) — check whether that is
  expected or the floor/margin needs a look.
- Player experience: non-swallowing push-to-talk with mouse buttons is out in v0.1.15; per-user volume
  numbers + remembered volumes and join/leave sounds are out in v0.1.18; next: rate-limit wrong room-code attempts
  on the server (rooms are now permanent).
- Later: UDP-blocked fallback (reliable stream), RED if field data shows burst loss, code signing, Phase 3.


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
   3. **AGC done (4 Nov, not released or field-tested):** `callout-audio/src/agc.rs`, own implementation
      (no dependency). Runs on the audio thread **after the VAD and the level meter, before the pre-roll
      history and the encoder**, so VAD thresholds, the calibration and the meter keep seeing the signal
      before the gain, while pre-roll frames and the mic test carry it. Level loop: speech frames only
      (`VadDecision.active`, not the hangover), level smoothed fast-up/slow-down, gain aims at −23 dBFS RMS
      (down τ 100 ms, up τ 1.5 s, 300 ms during the first 2 s of speech), range −10…+12 dB (+6 dB without
      noise suppression). Frames without speech never change the gain. The speech *peak* is tracked as well
      and caps the gain so peaks stay ≤ −6 dBFS; a quickly recovering limiter (separate from the gain) and
      a soft knee are the backstop; gain changes ramp over 2 ms. On by default (`Settings.agc`,
      `Command::SetAgc`, 11 languages); switching it on resets to unity gain; the log has an
      `agc gain_db limited_frames frames peak_out` line every 30 s while speaking. Client only: no protocol
      change, no server update needed.
      **First field test (4 Nov, Bera's webcam mic, release exe): crackling with AGC on, better with it off.**
      The first version pegged at +15 dB, aimed at −20 dBFS and wrote every limiter hit into the gain
      (instant drop, 1.5 s recovery = pumping). Reworked (target −23, ceiling +12, peak tracking, separate
      limiter, 2 ms ramp, limiter stats in the log).
      **Second check (4 Nov, reworked version, one-off local recording of the mic path):** the AGC is clean. Measured on a 50 s
      recording: gain steady at +12 dB, non-linear residual after removing the per-frame gain −152 dB
      (median), no clipped samples (peak −15.7 dBFS), no capture dropouts (`in_overrun=0`, no zero gaps).
      The "crackle" Bera heard turned out to be mouth noises (clicks at the same times in the raw
      recording), now 12 dB louder; by ear (Bera) the denoised + AGC version sounds fine. Side findings:
      the webcam mic's SNR is only ~9 dB (floor −42, loudest speech frame −33 dBFS), and RNNoise cuts deep
      inside words there (p10 −68 dB on the loudest frames); with noise suppression **off** the VAD
      threshold (−36.6) sat above nearly all speech, so the mic test played nothing.
   4. **Automatic reconnect done (4 Nov, v0.1.12, field-tested by Bera):** after an involuntary disconnect
      (QUIC connection lost / ping failed) the engine enters `ConnectionState::Reconnecting`, retries with 1, 2, 4,
      8, 15 s… backoff (quietly, no error per attempt), rejoins the previous room and restates mute/deafen
      (also fixed for fresh joins). `Disconnect` stops it; a user `Connect` starts fresh. If the room is gone
      (everyone left, server restarted) the server answers RoomNotFound and the user lands in the lobby.
      End-to-end test `engine_reconnects_after_server_restart_and_tries_to_rejoin` kills and restarts the server.
      Rejoining a room that survived (only this client dropped) is not covered by a test yet.
   5. Fallback for UDP-blocked networks: a mode that carries audio packets over a reliable QUIC stream.
   6. Lossy network simulation test (5% loss, 100 ms jitter) — headless. Jitter part done
      (`tests/jitter_sim.rs`). The loss part is in
      `callout-audio/tests/loss_sim.rs`; the jitter (delay variation) part remains.
   7. UI polish: done (VAD sensitivity slider with dB readout/reset, listen-only join, Bluetooth mic warning,
      panic logging, icons). **Mic test done (2 Nov,
      v0.1.8, tried in the app):** a "Mic test" section in settings — 5 s recording, played back the way
      others in the room would hear it (VAD/push-to-talk gate + Opus) along with the sent share; "Play test sound"
      (a chime); a speech-detection threshold line on the level meter (also on the in-room meter). When not connected
      and settings are open, the engine opens a local audio pipeline (`SetAudioPreview`); the selected device is
      applied without saving, "Cancel" reverts it. No audio goes to the room during recording.
3. **The push-to-talk shortcut was swallowing the key** (28 Oct, found in the two-person test): the shortcut was
   registered system-wide from app start, so a plain "T" could not be typed in other apps.
   Fixed: registered only while push-to-talk is on **and** in a room (`sync_ptt`). Outside a room
   (including the tray) the key is free. **Permanent fix done (4 Nov):** `callout-input` listens with Raw
   Input (not a WH_KEYBOARD_LL hook, which would sit in the input path), never consumes the key, and supports
   mouse buttons; checked in a game.
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
7. Known gaps: closed 4 Nov (Raw Input push-to-talk works in a fullscreen game, mouse buttons supported).

## Workflow

- **Version number:** the tag is the single source. `release.yml` writes the tag (`v0.1.5` → `0.1.5`) into
  `Cargo.toml` `[workspace.package] version` before building; the installer name
  (`CallOut_0.1.5_x64-setup.exe`) and the UI footer (`app.package_info()`) come from there.
  `tauri.conf.json` has no `version`. The workflow fails if the tag is not in `vX.Y.Z` form. Occasionally sync the
  `Cargo.toml` version in the repo with the latest tag (the version shown in local runs).
- Server update: push a tag (`git tag v0.1.x && git push origin v0.1.x`), and once Actions finishes run
  `curl -fsSL https://raw.githubusercontent.com/berabozgoz/callout/main/deploy/install.sh | sudo bash` on the server.
- macOS test DMG without a release: open a PR (or Actions → CI → "Run workflow"); the `dmg-macos` job
  uploads `CallOut-macOS-<sha>` (a zip with the universal DMG, kept 7 days). It has no updater bundle.
- After code changes: `cargo fmt && cargo clippy --workspace --exclude callout-app --all-targets -- -D warnings && cargo test`.
- If the protocol changes: `callout-proto` + `PROTOCOL_VERSION`; client and server are updated together.

## Last session

- 11 Nov (macOS app, owner's request "release CallOut for macOS outside the App Store"; owner chose:
  unsigned for now, push-to-talk in the first Mac release, testing on an Apple Silicon Mac):
  `callout-input` got `mac.rs`, a listen-only Quartz event tap (objc2-core-graphics/-foundation, already in
  the tree via Tauri) on its own thread with a 100 ms run-loop slice (commands, "still held?" check via
  `CGEventSourceKeyState`, retry until Input Monitoring is allowed; the tap is re-enabled when macOS
  disables it), and `mac_keys.rs`, the `kVK_*` ↔ set-1 scan code table following WebKit's
  `KeyboardEvent.code` mapping (tested on every platform). `has_input_access` / `request_input_access`;
  app commands `ptt_input_access`, `open_input_monitoring_settings`; UI shows a warning with a button
  while access is missing (rechecked on window focus), a Mac variant of the push-to-talk hint (no
  administrator sentence) and Cmd/Option key names; 3 new keys in all 11 languages. `callout-priority`:
  macOS joins QoS user-interactive (`libc::pthread_set_qos_class_self_np`). App: log at
  `~/Library/Logs/io.github.berabozgoz.callout/`, `is_installed` = inside an `.app` not under `/Volumes`,
  Dock click reopens the hidden window (`RunEvent::Reopen`), log scrub test covers `/Users/<name>/`.
  Bundle: `tauri.macos.conf.json` (app + dmg, macOS 11+, ad-hoc `signingIdentity "-"`, hardened runtime,
  `Entitlements.plist` with audio-input), `Info.plist` (microphone text). CI: `app-macos` job; release:
  `macos` job after the Windows job (universal target; tauri-action merges the darwin entries into the
  existing `latest.json`, checked in its source); `deny.toml` covers the Apple targets. README, CLAUDE.md,
  ARCHITECTURE updated. Windows: fmt/clippy (incl. app)/tests/svelte-check/`cargo deny` clean; macOS: only
  the two platform crates were checked (from Windows); nothing has run on a Mac.

- 8 Nov: **v0.1.19 released** (echo cancellation, adaptive output reserve, latency diagnostics; app only,
  server stays on v0.1.16). Release and CI green; six assets (installer + `.sig`, portable exe, both
  servers, `latest.json` serving 0.1.19); release notes carry a "What's new" that tells players everyone
  in the room has to update for the echo to go away. Next: a game session with kuzen and Hallobadi on
  v0.1.19 (echo gone? `echo` and `audio latency` lines in their logs) and the other v0.1.19 items in
  "Pending manual tests".

- 8 Nov (echo cancellation, Bera's request: he heard his own voice from kuzen's and Hallobadi's
  headphones): new `callout-audio::aec` wraps `sonora` 0.3 (pure-Rust port of WebRTC audio processing,
  BSD-3-Clause, `cargo deny` clean), echo canceller only (RNNoise/VAD/AGC stay ours). The engine feeds the
  final mix (voices, cues, test sounds; what the device plays) as the render reference and runs the
  canceller first on every microphone frame (before RNNoise, VAD, AGC); the delay hint is the output
  ring's fill + both device buffers + 10 ms (AEC3 finds the real delay itself). On by default
  (`Settings.echo_cancellation`, `Command::SetEchoCancellation`, `EngineConfig.echo_cancellation`; older
  settings files get it on), switching restarts it; talk example `--no-aec`. Log every 30 s while
  something played: `echo erl_db erle_db delay_ms`. UI: "Noise suppression" and "Echo cancellation" as two
  short toggles on one line (details as tooltips) so the settings page does not grow; 11 languages.
  `tests/echo_sim.rs` (synthetic speech, delayed reverberant echo path): speakers −12 dB echo 41 dB
  removed, a −32 dB headphone leak ends below the mic noise, the player's own voice −0.3 dB alone and
  −1.5 dB during double talk, 38 dB echo removed in pauses after ~25 s of constant double talk; CPU
  0.008 s per s of audio (release). sonora needs Rust 1.91: workspace `rust-version` 1.80 → 1.91, which
  turned on two clippy lints (`as_chunks`, `is_multiple_of`; fixed). fmt/clippy (incl. app)/98 + 7 app
  tests/svelte-check/`cargo deny` clean. Local smoke run (one client, release): pipeline fine with AEC on
  (loop max 3 ms), `echo delay_ms=76` vs a hint of ~72. Not tried with real echo yet (needs speakers or a
  friend), not in the app UI; released in v0.1.19.

- 8 Nov (latency diagnostics + adaptive output reserve, Bera's go-ahead after the latency discussion): the
  engine mixed a new frame only when the output ring held < 40 ms, so ~40–60 ms (avg ~43–50) of decoded
  audio always waited for the device, more than the jitter buffer adds on a wired link. Now
  `timing::OutputReserve`: starts at 30 ms, after 120 s without underruns tries 10 ms less (min 20), on any
  underrun +10 ms at once (max 80); a value that fails within 60 s of being tried is not tried again for the
  pipeline's life. Steps are 10 ms because the device takes ~10 ms periods (a 5 ms step changed nothing in
  a local run). Underruns in the first 2 s are ignored (ring filling). New log lines: `output reserve`
  (changes) and every 30 s `audio latency reserve_ms out_fill_ms out_device_ms in_device_ms` (device
  figures from cpal's callback timestamps, one atomic store in the callback); `sender jitter` gained
  `wait_ms` (average time played packets spent in the jitter buffer; lone packets > 100 ms after the
  previous one, e.g. DTX comfort noise every 400 ms, are skipped). `out_underrun` is no longer reset by the
  log (both readers take deltas). Tests: 4 reserve, fill meter, device latency, 3 jitter wait tests;
  fmt/clippy (incl. app)/95 tests clean. Local runs (release talk example, local server, Bera's PC):
  MMCSS on, no underruns after start-up, reserve 30 → 20 after 2 min, `out_fill_ms` 33 → 24, output
  device 10 ms, input device 5–19 ms, `wait_ms` 19–66 with target 40. The two-client run howled through the
  speakers (both clients on one PC's mic and speakers, push-to-talk held): use one client, or headphones /
  separate devices, for local tests. Not tried in the app UI or a game; released in v0.1.19.

- 8 Nov: **v0.1.18 released** (audio thread MMCSS priority + `audio timing`, per-user volume numbers and
  remembered volumes, join/leave sounds; app only, server stays on v0.1.16). Release and CI green; six
  assets (installer + `.sig`, portable exe, both servers, `latest.json` serving 0.1.18); release notes carry
  a "What's new". Next: a game session with friends on v0.1.18 → compare `out_underrun` / `late` /
  `audio timing` with 7 Nov, and the three v0.1.18 items in "Pending manual tests".

- 8 Nov (join/leave sounds, Bera's request): `callout-audio::cues` synthesizes four styles (Chime, Pop
  "bubble", Marimba, Beep "radio"), each with a rising join and a falling leave form (48 kHz, frame-aligned,
  peak 0.22, faded ends; no sound files). The engine plays the join cue when we enter a room and when a
  member joins (`MemberJoined` for someone not yet listed), the leave cue on `MemberLeft`; not when deafened
  or Off. The audio thread renders and mixes it like the test sound (`cue_request` atomic, never in a cpal
  callback). New `Command::SetJoinSound` / `PreviewJoinSound`, `JoinSound` (default Chime) in settings and
  `EngineConfig`; Settings page row with a select and a play button (join, then leave), 11 languages.
  Tests: cue shape (3), request encoding/deafen/off/serde (3), settings default (1); clippy, svelte-check
  clean. End-to-end with two talk clients on a local server: own join, other's join and other's leave
  (after a dropped connection) all triggered. Settings page checked in the browser preview (fits 750 px).
  Not heard/picked in the real app yet, not released.

- 8 Nov (per-user volume, Bera's request): the room shows each player's volume as a number next to the
  slider (localized: `%55` in Turkish, `55 %` in German/French…; highlighted when not 100%; a click resets
  to 100%). Volumes are remembered across sessions by **nickname** in `settings.json` (`user_volumes`, only
  non-100% values): the protocol has no stable user id (a `user_id` lasts one connection), and adding the
  device key to `Member` would need a protocol bump and a server update. The app owns the map: it saves on
  every slider change (`send_command` looks up the nickname in the snapshot), resends saved volumes to the
  engine on every `RoomUpdated` (covers join, rejoin and reconnect, which all bring new user ids), and
  `save_settings` keeps its own copy over the UI's. 3 new app tests; clippy, tests, svelte-check clean;
  UI checked in the browser preview with a mocked Tauri layer (saved 50% shown, slider updates the
  number, reset sends gain 1). Not tried in the real app, not released.

- 8 Nov (audio thread priority, from the LoL/CS2 findings): new crate `callout-priority` (unsafe, rule 6;
  `windows-sys` Threading only, already in the tree) puts the engine audio thread into MMCSS "Pro Audio"
  for the thread's lifetime (reverted on exit), fallback `THREAD_PRIORITY_HIGHEST`. cpal's own WASAPI
  callback threads were already time-critical. `timing.rs` in callout-core: `GapMeter` (atomics only,
  ticked in both cpal callbacks) and `LoopMeter`; one `audio timing` info line every 30 s. The mix
  branch now sleeps 2 ms when nothing reached the output ring, so a raised-priority thread cannot spin
  on a resampler error. Tests (5 new), clippy (incl. app), cargo deny clean; tried with the talk example
  against a local server (MMCSS granted, loop max 3 ms). Not tried in the app UI or a game, not released.

- 7 Nov (LoL field test, Bera + Hallobadi, v0.1.17, both AGC on; results in `GATE_TEST.md`): ~50 min,
  no drops; RTT + buffer ~100 ms both ways; FEC covered 92% of Hallobadi's ~0.5% uplink loss; Bera's
  webcam AGC gain ≈0 dB (the retune works). Found: playback underruns on Hallobadi's PC under LoL
  coinciding with his `late` bursts → audio thread starved by the game (see Next). Players: Hallobadi
  found Bera's voice good (not too loud), Bera noticed no dropouts; Hallobadi tried push-to-talk for a while.

- 4 Nov: **v0.1.17 released** (AGC comfort band, off by default with a one-time switch-off; app only, server
  stays on v0.1.16); `latest.json` serves 0.1.17, release notes carry a "What's new". During the build Bera's
  PC rebooted with bugcheck 0x00020001 (HYPERVISOR_ERROR, no dump written): Windows' virtualization layer,
  not CallOut (user mode only, no driver); watch whether it repeats.

- 4 Nov (CS2 field test, Bera + kuzen, v0.1.16, results in `GATE_TEST.md`): log upload works on the real
  server; a 10 s outage on kuzen's line was recovered by the automatic reconnect in 8 s (room and mute kept);
  Bera's app close reached the server as `bye`. kuzen → Bera was irregular under CS2 (Bera's buffer avg
  108 ms, p95 240; the other way avg 42 ms); sender side (uplink or audio thread under load) cannot be told
  apart yet → next: capture-timing stats on the sender. FEC covered ~77% of the loss; RED not needed.
  Both players turned AGC off (better without; Bera too loud with it).
- 4 Nov (AGC retune, from the CS2 test): `agc.rs` swaps the −23 dBFS target for a comfort band
  (`BAND_LOW_DB` −30, `BAND_HIGH_DB` −20): no gain inside it, a quieter mic is raised only to −30, a louder
  one lowered to −20 (peak cap, limiter and ramps unchanged). New tests: a mic in the band stays at unity
  (−29/−26/−21 dBFS), a boosted quiet mic returns to unity when it gets louder; the old tests moved to a
  −40 dBFS "quiet" level. AGC is now **off by default** (`Settings`, `EngineConfig`, UI default, talk
  example); `turn_off_old_agc_default` switches it off once for older settings files, a later choice is
  kept; `save_settings` keeps the internal flags (`installer_language_seen`, `agc_default_off_seen`) from
  the stored copy. fmt/clippy (incl. app)/test/svelte-check clean; not tried in the app, not released.

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
- 4 Nov (automatic reconnect): see Next → Phase 2 item 4; UI shows a "Reconnecting…" card with a Stop button
  (11 languages). fmt/clippy (incl. app)/test clean; not tried in the app, not released.
- 4 Nov (housekeeping): voice activity sensitivity slider (d16c116); `deny.toml` + CI job `cargo deny check
  licenses sources` (MPL-2.0 allowed only for five transitive Tauri crates: cssparser, cssparser-macros,
  dtoa-short, selectors, option-ext); full Apache-2.0 text in `LICENSE-APACHE`; README rewritten for players,
  server hosts and developers; release notes template now covers the Linux server (`install.sh`).
- 4 Nov (auto-update): `tauri-plugin-updater` (MIT/Apache) with `createUpdaterArtifacts`; endpoint
  `releases/latest/download/latest.json` (tauri-action `includeUpdaterJson`); minisign public key in
  `tauri.conf.json`, private key + password outside the repo (location in `CLAUDE.local.md`) and as GitHub
  secrets `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)`. The app checks at start and every 6 h (release builds only)
  and shows a banner; "Update and restart" downloads, disconnects cleanly, then runs the NSIS installer in
  passive mode. The first updater-enabled release has to be installed by hand once. The portable exe
  (no `uninstall.exe` next to it) gets a "Download" button that opens the Releases page instead.
- 4 Nov (persistent rooms): the server keeps empty rooms (same code later) and saves them to
  `<data>/rooms.tsv` (`code<TAB>last_used<TAB>name`, write-then-rename); rooms unused for 30 days expire (hourly
  check), at most 1000 rooms. After a server restart reconnecting clients land back in their room (the
  reconnect end-to-end test now asserts this). The lobby lists the last 5 rooms (`Settings.recent_rooms`) for
  one-click rejoin. Needs a server update to take effect; protocol unchanged.
- 4 Nov (close room): the room's creator can close it for everyone and delete it from the server. Ownership
  is the creator's device key (kept in `rooms.tsv` as a 3rd column, so it survives reconnects and restarts);
  `RoomInfo.owner` carries the owner's current session id (0 if away), which is how the UI shows the button.
  Protocol: `ClientMsg::CloseRoom`, `ServerMsg::RoomClosed { code }`, `ErrorCode::NotOwner` appended (after
  `Internal`, since postcard encodes variant indices); `PROTOCOL_VERSION` stays 1, but a pre-v0.1.12 client in
  a room that gets closed cannot decode `RoomClosed` and drops its connection. UI: "Close room" with a
  click-again confirmation; closed or vanished rooms leave the recent list. E2E test
  `only_the_creator_can_close_a_room`.
- 4 Nov (manual tests by Bera): reconnect, listen-only, device recovery, persistent rooms and close room
  passed (see Pending manual tests). Found: while the server is down each reconnect attempt waited for the
  full 10 s QUIC idle timeout, so attempts ran every ~25 s; attempts are now capped at 4 s
  (`RECONNECT_ATTEMPT_TIMEOUT`). Reconnect keeping mute and the Stop button also passed: all
  "before release" tests are done.
- 4 Nov: **v0.1.12 released** (first signed release: `CallOut_0.1.12_x64-setup.exe.sig` and `latest.json`
  with `windows-x86_64` entries; the updater endpoint resolves) and the server updated to v0.1.12
  (`rooms loaded`, invite link unchanged). Everyone installs v0.1.12 by hand once; later releases arrive in-app.
  Next field checks: auto-update with v0.1.13, persistent rooms / close room on the real server.
- 4 Nov (installer language → app): the NSIS installer stores the picked language as an LCID in
  `HKCU\Software\berabozgoz\CallOut\Installer Language`; at start the app maps it (1055 → tr, 1033 → en, …) and
  sets `Settings.language` once per new choice (`installer_language_seen`), so a later in-app change is kept.
- 4 Nov: **v0.1.13 released** (installer language → app language, icons in settings and the sound test) and
  delivered through the in-app updater: Bera's v0.1.12 updated itself to v0.1.13. The update path works end to end.
- 4 Nov (UI polish after v0.1.13): settings device pickers and the sound test buttons use the microphone /
  headphones icons (emoji removed from those texts), "Speaker" renamed "Audio output"; the sensitivity slider
  shows its value in dB and has a "Reset to default" link (4dfc354, unreleased). Session handed over to a new
  conversation here; see "Where we left off" and "Start here next session".

- 4 Nov (AGC crackle investigation): dropout counters in the cpal callbacks (atomic add only; log
  `audio dropouts (samples) in_overrun out_underrun` every 30 s when non-zero; a few hundred to ~1500
  output samples right after the pipeline starts are normal). For the analysis a temporary debug build wrote
  the mic path stages to WAV files on Bera's PC; result: the AGC is not the cause (see Phase 2 item 3).
  That recorder was then **removed from the source on the owner's decision: CallOut never writes
  microphone audio to disk**, not even behind a feature flag (rule 9 in `CLAUDE.md`); it is kept as a
  local-only patch outside the repo for future tests. fmt/clippy/test clean.
- 4 Nov (UI fixes before v0.1.14, from Bera): the settings page no longer scrolls: devices and language
  are caption + control on one row (`.formgrid`), the push-to-talk key too, the sound test is tighter, and
  the window is 780 px tall (was 640), centered, shrunk to the monitor's work area on short screens
  (`fit_window_to_screen`). Measured in the browser preview at a 748 px client height: every language fits
  in both modes (longest: Japanese with push-to-talk, 746 px). "Close room" now opens a confirmation strip
  (`room.closeAsk`) instead of turning into a long "click again" button that pushed the room code around.
  **No default push-to-talk key any more** (F9 pauses Dota 2, every key clashes with some game): turning
  push-to-talk on asks for one; a stored `F9` with push-to-talk off is cleared at start
  (`forget_old_default_ptt_key`); the room warns when push-to-talk has no key (`room.pttNoKey`); the hint
  says to pick a key the game doesn't use. The sensitivity slider stays hidden with push-to-talk: the VAD
  is not used then (the key is the gate; the mic test records without it). 11 languages updated.
  clippy (incl. app)/test/svelte-check clean; checked in the release exe by Bera.
- 4 Nov (non-swallowing push-to-talk): new crate `callout-input`, the only one with `unsafe` (rule 6):
  Windows Raw Input (`RIDEV_INPUTSINK`) on a hidden window of its own thread replaces the Tauri global
  shortcut (`RegisterHotKey`, which consumed the key system-wide; the plugin and its permissions are
  removed). The key or mouse button is only observed, so it still works in the game. Bindings keep the old
  format (`KeyboardEvent.code` + modifiers) and add `Mouse3/4/5` (middle, back, forward); keys are matched
  by scan code (physical position, layout independent). Extra modifiers do not block a plain key; only the
  trigger's release ends the transmission. Devices are registered only while a binding is set (in a room,
  push-to-talk on, not capturing), the mouse only for a mouse trigger; a 250 ms timer releases if the
  trigger is no longer physically down (lost key-up on the secure desktop). Raw input registration is per
  process: this replaces tao's startup registration (device events, unused by Tauri). Known limit: no
  input while an elevated game has focus unless CallOut is elevated too. UI: the capture field takes mouse
  buttons; hint/capture texts in 11 languages; README. 10 unit tests (parser, press/release logic).
  fmt/clippy (incl. app)/test/svelte-check/cargo deny clean; checked in the release exe and in a game by
  Bera; released as v0.1.15 (release notes tell push-to-talk users that the key now also reaches the game).
- 4 Nov (log upload, Bera's idea): a player who agrees sends the app's diagnostic log to the server's
  operator, so field data no longer needs files passed around. Protocol: a **second bidirectional QUIC
  stream** after authentication carries `LogUpload` + the log text, answered by `LogUploadReply`
  (`callout-proto`); control messages are untouched, so an older server never accepts the stream (the
  engine gives up after 20 s → "this server can't receive logs") and older clients never open one; no
  `PROTOCOL_VERSION` change. Server (`logs.rs`): `<data>/logs/<UTC>_<nickname>_<device8>.log` with a header,
  ≤ 1 MiB, one per device per 10 min (in memory: a restart resets it), 30-day retention and a 200 MB cap
  (hourly), `CALLOUT_LOG_UPLOADS=0` turns it off. App: `send_log` reads the end (~1 MB) of `callout.log`,
  masks `\Users\<name>` in paths and 32+-digit hex runs, sends `Command::UploadLog`; answer
  `Event::LogUpload`. UI: "Send log" link bottom left while connected → consent strip (who gets it, what it
  holds, 30 days) → Send; after a reconnect or an audio/connection error the strip offers itself once per
  run. 11 languages; README (players + hosts), ARCHITECTURE, CLAUDE.md rule 9 note. Tests: proto
  roundtrip, server store/rate limit/cleanup/file names, scrubbing, end-to-end `engine_uploads_a_log_once_per_interval`.
  Checked by Bera in the release exe (old Oracle server and a local one). Released as **v0.1.16**; the
  Oracle server was updated to v0.1.16 the same day (`log uploads enabled=true`; the service may write
  `/var/lib/callout`, 42 GB free).
- 4 Nov: **v0.1.14 released** (AGC, sensitivity dB readout, settings without scrolling, close-room strip,
  no default push-to-talk key, dropout counters in the log). App only; the server stays on v0.1.12.
- 4 Nov (AGC): see Next → Phase 2 item 3. `VadDecision` gained `active`; seven unit tests in `agc.rs`
  (convergence, loud input without clipping, no change on non-speech frames, sudden shout, gain limits,
  reset, smooth ramp). fmt/clippy (incl. app)/test/svelte-check clean; not tried in the app, not released.

## Previous session

- 27–28 Oct: added Oracle (iptables) support to `deploy/install.sh`; set up the Oracle instance,
  added a public IP and a UDP 4433 ingress rule; the app connected to the server in the cloud.
