# CallOut — current status

> Read this file at the start of every session; at the end of a session update the "Last session" and "Next" sections.
> Private details such as the server address and invite link are not written here but in `CLAUDE.local.md`, which is not in git.

## Where we left off (23 Nov 2026)

- **Phases 0–1 done; Phase 2 (audio quality) mostly done:** FEC/PLC measured, noise suppression (RNNoise),
  adaptive jitter (delay-spread estimator + stall resync), VAD preroll/hysteresis/sensitivity, AGC (v0.1.14; retuned to a comfort band and
  off by default in v0.1.17 after the CS2 test). Left: the field checks below. Parts of Phase 4 are done early: in-app updates (signed, verified v0.1.12 → v0.1.13),
  panic logging; code signing (SmartScreen) is not.
- **Released:** v0.1.23 on GitHub Releases (23 Nov; macOS restarts after an in-app update, window not
  resizable, Windows header logo and arrow cursor on the title bar buttons, errors shown to the user are
  logged). v0.1.22 (18 Nov) was the redesign: new icon and theme, own Windows title bar,
  not maximizable, per-player mute. Each release has the Windows installer, portable exe,
  **macOS universal DMG** + updater bundle, both servers, `latest.json` with Windows and macOS entries). Installed
  copies from v0.1.12 on update in-app; older ones need one manual install of the latest release.
- **Server:** Oracle Always Free (Frankfurt) runs **v0.1.21** (protocol 2, Server Lobby; persistent rooms in
  `/var/lib/callout/rooms.tsv`, owner-only room closing, log uploads into `/var/lib/callout/logs/`; updated
  18 Nov 02:05 UTC, nobody was connected, invite link unchanged, 12 rooms loaded). Ubuntu
  `unattended-upgrades` is on (daily); rebooted 18 Nov 02:10 UTC for the pending kernel/`libc6` updates (now
  kernel 7.0.0-1016-oracle, no reboot pending; the `callout` service came back by itself). Check
  `/var/run/reboot-required` now and then. Details in `CLAUDE.local.md`; update with `install.sh` (see Workflow).
- **macOS app (released in v0.1.20, 11 Nov):** universal DMG (Apple Silicon + Intel, macOS 11+), outside the
  App Store, ad-hoc signed (no Apple Developer ID; users click "Open Anyway" once, and macOS may drop the
  microphone / Input Monitoring permission on every update). Push-to-talk via a listen-only Quartz event
  tap, audio thread at QoS user-interactive, in-app updates through the same `latest.json`. Tested on the
  owner's 2014 MacBook Air (Intel, macOS 11.7.11); no Apple Silicon Mac yet.
- **Unreleased on `main` (since v0.1.22):** macOS restarts by itself after an in-app update (`restart_app`); the
  header logo also on Windows, no icon in the title bar; errors shown to the user go to the log; the window
  is no longer resizable (19 Nov). Check `git log v0.1.22..`. The server needs no update for v0.1.22
  (no server or protocol change); it stays on v0.1.21.
- **Everything is English** (code, docs, logs, commits); the UI is localized in 11 languages (`ui/src/i18n/`,
  English is the source; add keys to every dictionary, the `Messages` type enforces it).
- **Product behavior worth knowing:** rooms are private (no server-side list; only people with the code, or who
  joined before, can find them), except the **Server Lobby** (v0.1.21): one room per server, always on the
  home screen, open to everyone connected, no code, no owner (nobody can close it), not joinable by typing
  a code; rooms persist until closed by their creator's device or unused for 30 days;
  the client reconnects and rejoins on its own; the app starts in the installer's language once per new
  installer choice; the portable exe gets a "Download" link instead of the in-app updater.
- The home network is behind CGNAT, so hosting the server from home is not possible; Tailscale was not wanted.

## Pending manual tests

Things that passed automated checks but still need a person, a real Windows PC or a real network.
Tick and move the result to "Last session" (or `GATE_TEST.md`) when done; release only after the
items marked **before release** pass.

- [x] **Server Lobby in the app** (before release; v0.1.21): with a local server
      (`cargo run -p callout-server`) and `cargo tauri dev`: after connecting, the home screen shows the
      "Server Lobby" card on top (in Turkish "Sunucu Lobisi"; check one more language and Arabic RTL),
      dark purple box `#2a2140` with a `#8634eb` frame and a lilac `#c9a7ff` title and a purple "Join" (owner's choice);
      "Join" enters it: title is the translated name, **no code button**, no "Close room"; "Leave" goes back
      and the lobby does **not** appear in the recent rooms; typing `LOBBY` in the code field says the room
      does not exist. Two PCs (or the app + `talk --lobby`) in the lobby hear each other. Kill and restart
      the server while in the lobby: the app reconnects and lands in the lobby again.
      ✓ 18 Nov (owner, `cargo tauri dev` on a local server): card shows with the final colours, joining and
      leaving the lobby several times works (server log: `joined lobby` / `lobby empty`). Checked by Claude
      in a mocked UI: Turkish, English, Arabic RTL, no code button/close, not in recent rooms, `LOBBY` by
      code refused. Two members hearing each other is covered by the e2e test (`lobby_is_open_to_everyone…`),
      not by ears; the reconnect into the lobby is untested in the app. Owner asked to release.
- [x] **Server Lobby with old/new versions** (at release): the new app against the **current Oracle
      server (v0.1.16)** connects normally and shows **no** lobby card (checked headless on 18 Nov with the
      talk example against a local v0.1.16 server: "server refused protocol 2 … retrying with 1",
      `lobby=false`; ✓ also live on 18 Nov 01:28 UTC: the dev app connected to the Oracle server with
      `version=1`, no card); after the release the server must be updated (`install.sh`), then an **old app**
      (v0.1.20) still connects and uses rooms by code. Release order: update the server right after the
      release is published (the lobby only shows once it runs protocol 2).
      ✓ 18 Nov 02:05 UTC: Oracle server updated to v0.1.21; the talk example (protocol 2) joined the live
      lobby (`lobby=true`, "in the server lobby").
      ✓ 18 Nov 02:12 UTC (owner, Windows): v0.1.20 on the updated server connected (`version=1`), entered
      three rooms by code, no lobby card; then the **in-app update** to v0.1.21 worked, the app connected with
      `version=2`, showed the lobby card, joined and left the lobby and a room by code (server log agrees).

- [x] **macOS: CI build** (before release): the `app-macos` CI job (clippy incl. the app, library tests on
      macOS) is green. Then a release dry run is the tag itself: the `macos` job must upload
      `CallOut_<v>_universal.dmg` and `CallOut_universal.app.tar.gz` (+ `.sig`), and `latest.json` must
      list `darwin-aarch64` and `darwin-x86_64` next to `windows-x86_64`.
      ✓ release part, 11 Nov (v0.1.20, run 43077641548): `CallOut_0.1.20_universal.dmg`,
      `CallOut_universal.app.tar.gz` + `.sig` uploaded; `latest.json` lists `darwin-aarch64`,
      `darwin-x86_64` (+ `-app`) next to `windows-x86_64`.
      ✓ CI part, 11 Nov (run 43052387197 on PR #1, head 0c4a453): `app-macos` (clippy incl. the app,
      library tests) and `macOS test DMG` green on the first try; the universal DMG artifact is ~12.9 MB.
      The release part waits for the first tag with the macOS job.
- [x] **macOS: empty window** (before release): 11 Nov, the first test DMG showed only an empty dark
      window on the owner's 2014 MacBook Air (Intel, macOS 11.7.11, Safari 16.6.1): CSS loaded, the script
      never ran (log: only `CallOut started`). The UI was built for `chrome105` only and contained private
      class methods (`#m()`); the app's WKWebView evidently runs an older WebKit than the installed Safari.
      ✓ 11 Nov: with the Vite target `["chrome105", "safari14"]` (DMG dcb1227) the UI shows on that Mac.
- [ ] **macOS with the release** (v0.1.20, owner's wish): (a) a proper listening test, Mac and Windows in
      different rooms (or one on headphones): voices clean both ways; (b) push-to-talk on a mouse side
      button (Mouse 4/5) on the Mac; (c) in-app update: the test DMG on the MacBook calls itself 0.1.19, so
      it should offer 0.1.20 → "Update and restart" replaces it in Applications (see the in-app update item
      below for what to watch: microphone / Input Monitoring permissions afterwards).
      ✓ (a) 14 Nov: "cakal" (a second Mac, v0.1.20, AirPods) + Windows "bera" (same home network) in room
      8LLBVGX7, ~4 min, both logs sent: voices clean (owner). Details in "Last session". Open: (b), (c).
      (c) partly seen in the field, 18 Nov: Furkan's Mac updated 0.1.20 → 0.1.22 (27 s between the two
      starts, so in-app with a manual restart, as expected) and the microphone still worked; macBera also
      reached 0.1.22 (how is not in the log). Push-to-talk after the update not checked.
- [ ] **Bluetooth mic warning at 24 kHz** (v0.1.21): AirPods on macOS
      capture at 24 kHz, which the old `<= 16000` limit missed; now `<= 24000` (`ui/src/App.svelte`). Check
      on a Mac with AirPods as the microphone: the warning shows "24 kHz" in settings and in the room; a
      48 kHz built-in or USB mic shows none (Windows AirPods at 16 kHz still warn).
- [x] **macOS: install and first start** (before release; Apple Silicon Mac): download the DMG (before a
      release: the `CallOut-macOS-<sha>` artifact of the "macOS test DMG" CI job on a PR or a manual
      "Run workflow"; unzip it), drag to
      Applications, start: blocked → System Settings → Privacy & Security → "Open Anyway" opens it (if
      macOS says "damaged" instead, `xattr -cr /Applications/CallOut.app` must fix it; note which one
      happened, it goes into the release notes). Microphone prompt shows the Info.plist text; after
      "Allow" the sound test shows a level and plays back. Settings, languages, tray (menu bar) icon work;
      closing the window hides it, clicking the Dock icon brings it back, Quit in the menu bar icon quits.
      ✓ 11 Nov (owner, 2014 MacBook Air, macOS 11.7.11): installs from the test DMG and opens via
      Privacy & Security; microphone allowed, talks with Windows; settings, Dock, quit fine (after the
      `safari14` UI target and the quit fix). Not yet: an Apple Silicon Mac.
- [x] **macOS: talk with Windows** (before release): Mac and a Windows PC in one room on the Oracle server
      for ~10 min: both hear each other, no crackle; the Mac log
      (`~/Library/Logs/io.github.berabozgoz.callout/callout.log`) shows
      `audio thread priority raised boost="qos user interactive"` and sane `audio timing` / `audio latency`
      lines (CoreAudio buffer sizes are new territory); echo cancellation on MacBook speakers.
      Logs ✓ 11 Nov (2014 MacBook Air "macBera" + Windows "bera", room 9RYJFSXX, ~6 min, both logs sent):
      Mac `boost="qos user interactive"`, device 44.1 kHz stereo in/out (resampled), `loop_max_ms` 4–7,
      no stalls, callbacks 12 ms; `out_device_ms=11 in_device_ms=11`, reserve settled at 20 ms
      (`out_fill_ms` 28): ~50 ms device side, lower than Windows. RTT 39–45 ms, no loss after the join
      (6/8 packets lost right at the join on both sides), Windows saw `late=0 underflow=0` from the Mac.
      ✓ (a) sound clean to the owner (11 Nov; a proper listening test with the release, two rooms apart); (b) the Mac's first 30 s had `out_underrun=119808` samples
      (~1.4 s) and `VAD calibrated floor=-100`, before joining a room: probably the microphone permission
      prompt (no input while it was up); check that a second start has no such underrun.
      Second log (4 more starts, 17:31–17:35): not the prompt. Every start has `VAD calibrated floor=-100`
      (the built-in mic delivers digital silence for its first ~1.5 s; threshold ends at the -60 dB clamp,
      as on Windows, so harmless but the calibration means nothing on this Mac) and a smaller start-up
      underrun (512–28160 samples). The owner found the Mac mic quiet; AGC on made it better (+7…+11 dB).
      Decided (owner, 11 Nov): AGC stays off by default on macOS too.
- [x] **macOS: quitting leaves the server politely** (before release; since the commit after 5bc0a37): all
      5 Mac quits on 11 Nov ended as `close_reason=TimedOut` on the server (Windows: 62 of 63 `bye`), so
      the old connection stayed in the room ~5–10 s (members=3 after a restart). Cmd+Q, the app menu and the
      Dock's Quit skipped the tray "Quit" handler that sends the close frame; `RunEvent::Exit` now does it
      too. Check: quit the Mac app with Cmd+Q while in a room; the server log shows
      `close_reason=...ApplicationClosed...bye` and the others see the leave at once.
      ✓ 11 Nov (DMG 19fe0b7): two Mac quits, both `ApplicationClosed ... bye` on the server, Mac log
      `closing connection reason="user disconnected"` first.
      Also seen with that DMG (it replaced the previous build in Applications): Input Monitoring was lost.
      Every start logs `push-to-talk needs Input Monitoring access; asking macOS` and never
      `listener active`: the ad-hoc signature changed, so macOS no longer matches the old entry. This is
      what every in-app update will do to push-to-talk until there is an Apple Developer ID (fix for the
      user: Input Monitoring → remove CallOut with "−", reopen CallOut, allow again).
- [x] **Redesign: own title bar on Windows** (v0.1.22; released before this was tried, owner's call): `cargo tauri dev` on Windows. The window
      has no system frame (`tauri.windows.conf.json`, `decorations: false`); the plum strip on top has the
      icon, minimize and close (**no maximize**: the window is `maximizable: false`, owner's wish). Check:
      dragging the strip moves the window, a double click does nothing, Win+Up and dragging to the top edge
      do not maximize, minimize works, **close hides to the tray** like before (tray "Show" brings it back;
      "Quit" quits). The window can be **resized from its edges** and still has its shadow. macOS: the
      native title bar stays (no plum strip); the green button must not zoom or go full screen.
      ✓ 18 Nov (owner, released v0.1.22 after the in-app update from v0.1.21): the new window and icon came
      with the update, the window cannot be maximized; Win+Up does not maximize it either, and the window
      resizes from its edges (owner first kept that; on 19 Nov asked to turn it off, see below).
- [x] **Redesign: per-player mute** (v0.1.22; not tried before the release): in a room with two people, the speaker button next
      to the other player's slider mutes them for you only (red icon, slider locked, the percentage
      struck through, no speaking ring); unmuting brings back the same volume (try at 50 %). Pulling the
      slider to 0 shows the speaker as muted; the speaker then brings it back to 100 %. The mute survives
      leaving and rejoining the room and an app restart (`muted_users` in `settings.json`), and the
      saved volume is not overwritten with 0.
      ✓ 18 Nov (owner, v0.1.22): muting works in a room and is still there after a restart.
- [x] **Redesign: look** (v0.1.22; tried by the owner on Windows in `cargo tauri dev` before the height/no-maximize change; the Mac is open): connect, home (Server Lobby card, recent rooms), room, settings
      (switches, groups) in the real app on Windows and on the old Mac (Safari 14 WebKit: switches drawn,
      no blank page). New icon on the taskbar, tray (16 px), installer, Dock. **Settings fit without
      scrolling** at the default size (Windows 420×920 incl. the bar, macOS 420×890), with push-to-talk on
      and off (measured in the mocked UI for all 11 languages; the longest is Japanese with push-to-talk,
      915 px). On a screen shorter than that the window shrinks to the work area (now
      `create_main_window`) and the settings scroll again.
      ✓ 18 Nov (owner, v0.1.22): settings fit, looks right on Windows and on the Mac.
- [ ] **Window not resizable** (v0.1.23; 19 Nov, owner's wish): `resizable: false` in `tauri.conf.json`
      and `tauri.windows.conf.json` (it was already not maximizable). Check on Windows and the Mac: the
      window edges no longer resize it (no resize cursor), dragging the title bar still moves it, minimize
      and close work; settings still fit at 420×920 / 420×890. On a screen shorter than that the window
      still starts shrunk to the work area (e.g. Windows display scaling 150 % on 1080p) and the settings
      scroll there.
      ✗ 23 Nov (owner, v0.1.23 on the 2014 MacBook Air): at the first start the header (Settings button)
      sat too high, under the title bar; after moving the window it was right. Fixed after v0.1.23, see
      the next item.
- [x] **macOS: first start on a short screen** (next release; found 23 Nov): on the 2014 MacBook Air
      (screen shorter than 890 + title bar) start CallOut: the window starts shrunk to fit above the Dock,
      the header and Settings button sit right under the title bar at once (no need to move the window),
      the settings scroll to the end. Also on Windows: the window still opens centered at 420×920 (✓ 23 Nov
      debug build on a 1920×1080 screen, by Claude); at 150 % scaling on 1080p it starts shrunk.
      ✓ 23 Nov (owner, CI test DMG of 391ae88 on the 2014 MacBook Air): the window is smaller and has the
      same size from the first start on. The settings scrolled there; the owner first kept that, then
      asked for a compact layout (next item). Not yet seen: Windows at 150 % scaling.
- [ ] **macOS: compact settings on a short screen** (next release; 23 Nov, owner's wish): on the 2014
      MacBook Air (Dock at the bottom) open Settings: everything down to Save/Cancel shows without
      scrolling, with push-to-talk off and on (also while capturing a key, the tallest case), in Turkish
      and one longer language (Spanish or Japanese); text still readable, nothing clipped. On a Mac whose
      window has the full 890 pt the settings look as before. Windows unchanged.
      ✗ 23 Nov (owner, CI test DMG of f566294): fits in every language except **Spanish with push-to-talk
      on**, which still scrolled. Tightened again after that (see "Last session"); check Spanish (and
      Japanese, now the tallest) once more with the next test DMG.
- [ ] **macOS: restart after an in-app update** (in v0.1.23, visible from the update after it; found 18 Nov): updating the Mac to v0.1.22
      downloaded and installed it, but the app stayed at "100%" and only a manual restart started the new
      version: on macOS the updater replaces the bundle without relaunching (the Windows installer restarts
      the app itself). Fixed after v0.1.22: the UI calls the new `restart_app` command (`AppHandle::restart`)
      right after `install()`. The running version's code does the update, so **the update to the next
      release still needs a manual restart on the Mac**; the fix shows from the update after that
      (v0.1.23 → later). Check then: "Update and restart" relaunches CallOut by itself, the new version
      shows in the footer; on Windows nothing changes (the installer restarts it as before).
- [x] **macOS: Dock icon size** (since the commit after 94175fe): the owner saw CallOut larger than the other
      Dock icons (the icon filled the whole canvas). Now `icons/icon.icns` on Apple's grid (824 px body in
      1024, soft shadow; made by `make_mac.py` outside the repo). Check: same size as its Dock neighbours.
      ✓ 11 Nov (owner, DMG 736d8a6).
- [x] **macOS: flat drop-downs** (since the commit after cd79424): WKWebView drew the selects (microphone,
      audio output, join sound, language) as glossy Aqua controls. On macOS (`html.mac`, set in `main.ts`)
      they now use `appearance: none` with the app's colours and a gradient arrow (checked in the browser
      preview, also right-to-left; Windows unchanged). Check on the Mac: flat like on Windows, the list still
      opens and picks.
      ✓ 11 Nov (owner, DMG 736d8a6). The test DMG no longer opens the Web Inspector (CI dropped
      `--features devtools`; the feature stays for later diagnosis), so it matches the release build.
- [x] **macOS: push-to-talk** (before release): turn on push-to-talk in Settings: the yellow "Input
      Monitoring" note with its button shows; the button brings the macOS prompt / System Settings with
      CallOut listed; after allowing and reopening CallOut the note is gone. In a room: holding a key
      (also a combination with Cmd/Option, shown as "Cmd"/"Option") and a mouse side button transmits only
      while held, the key still types in other apps, and nothing sticks after release (also when releasing
      inside a password field). The log has `push-to-talk listener active`.
      ✓ 11 Nov (owner, 2014 MacBook Air): works; log shows `push-to-talk needs Input Monitoring access;
      asking macOS`, then after reopening `push-to-talk listener active mouse=false`. Not yet tried: a
      mouse side button, release inside a password field.
- [ ] **macOS: in-app update** (after the first macOS release, with the next one): the update banner
      shows, "Update and restart" replaces the app in Applications and restarts it (18 Nov, v0.1.22: the
      update installs, but no restart; see "macOS: restart after an in-app update"). Check whether
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
- [ ] **Title bar cursor on Windows** (v0.1.23): hovering the minimize and close buttons in the own title
      bar keeps the arrow cursor (no hand), like native caption buttons; hover colours unchanged.

## Next

**Start here next session** (pick with the owner):
- **From the 21 Nov CS2 session (see "Last session"), candidates:** (a) jitter target tuning: it jumps
  ×4–6 on a few late packets and drops back to 40 ms too soon, so kuzen's later late packets became
  underflows; replay the 21 Nov numbers in `tests/jitter_sim.rs` before changing it (fits plan steps 3/5
  below); (b) re-probe the microphone while listen-only, so a re-plugged headset works without restarting
  the app (check first whether the engine already retries); (c) fall back to the default device when the
  saved input/output name is gone (and tell the user); (d) device names in the
  `audio pipeline started` line would have made the diagnosis direct; ask the owner first, since the
  19 Nov change deliberately keeps device names out of logged messages (they reach uploaded logs).
- **Errors shown to the user are now logged (released in v0.1.23):** the next field log with a failed audio start or connect should say why (see "Last session").
- **Redesign (released in v0.1.22):** the owner's tests passed (update, window, icon, mute, settings fit,
  Mac look). Left: the Mac did not restart after the in-app update; fixed (`restart_app`) and released in
  v0.1.23 (the Mac's update to v0.1.23 still needs a manual restart; first visible on the update after that).
  Source of the icon: `make_flat.py B` in the icon work folder outside the repo.
- **Server Lobby (released in v0.1.21, 18 Nov; old/new versions checked by the owner):** left is the lobby
  with friends (several people talking in it). Ideas the owner may want later (not built): member count of the lobby on the home screen before joining (needs
  a new server message), a server setting to turn the lobby off for self-hosters.
- **macOS app (released in v0.1.20):** next the open macOS items in "Pending manual tests" on the owner's Apple Silicon Mac,
  then release. Known gaps: no Apple Developer ID (Gatekeeper "Open Anyway", permissions may reset on
  update; signing + notarization are prepared in `release.yml` comments), a denied microphone gives
  silence instead of the "microphone access denied" error (needs an AVCaptureDevice check); Caps Lock cannot be a push-to-talk key on macOS.
  AirPods on macOS capture at **24 kHz**; the Bluetooth mic warning now fires up to 24 kHz (unreleased,
  see Pending manual tests).
- **Jitter / latency work (started 8 Nov, released in v0.1.19):** steps 1–2 of the plan are done (latency log,
  adaptive output reserve). Next, once field logs show the breakdown: (3) an ms-precise jitter target instead
  of `ceil(spread/20)+1` frames (~10–15 ms on calm links; `tests/jitter_sim.rs` to measure), (4) time
  stretching (WSOLA, own code or an Apache/MIT library; SoundTouch is LGPL) to shrink/grow the buffer
  mid-spurt and absorb clock drift, (5) faster recovery after a delay spike (the 20 s window keeps the target
  high). Later: 10 ms Opus frames, WASAPI low-latency shared mode (IAudioClient3).
  Canada test (16 Nov, see "Last session"): 118 ms RTT is fine; the sender's Wi-Fi stalls (0.5–1 s) cause
  most concealment and FEC cannot help there, so (5) matters more than (3) for such players.
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

- 23 Nov (owner): **Mac header too high at the first start, fixed (not released).** On the 2014 MacBook
  Air the v0.1.23 window opened with the header (Settings button) shifted up under the title bar until
  the window was moved. Cause: since `resizable: false`, macOS no longer shrinks a window that is taller
  than the screen when it is created, and our later shrink (`fit_window_to_screen`, `setContentSize`
  async on macOS) left the page laid out for the old size (v0.1.22, still resizable, looked right on the
  same Mac). Fix: the main window has `create: false` in both configs and `create_main_window`
  (`apps/callout-app/src/lib.rs`) builds it in `setup` with a height that already fits the primary
  monitor's work area (minus a 28 pt title bar estimate when the window has decorations; never below
  `minHeight`), so it is never resized after creation. fmt, clippy (app, `-D warnings`), `cargo test -p
  callout-app` clean; a Windows debug build opens centered at 420×920. Pushed (391ae88); the owner tried
  the CI test DMG on the Mac: the window has the right size from the first start. The settings scroll
  there (see below).
- 23 Nov: **settings height measured** (mocked UI in Chromium, 420 px wide, without the Windows bar):
  push-to-talk off 726–775 pt (Turkish 726, Russian 775), on 814–883 pt (Turkish 814, Japanese 883).
  On a 1440×900 Mac with the Dock at the bottom the window is ~790 pt, so the settings scroll there with
  push-to-talk on in every language; 13.6" Air / 14" Pro are borderline; 15"/16", iMac and external
  screens fit. The owner first chose to keep the scroll, then asked for a compact layout (next entry).
- 23 Nov (owner: "compact the Mac window for small screens"): **compact settings on short Mac windows**
  (not released). `ui/src/app.css`: `@media (max-height: 889px)` scoped to `html.mac` (set in `main.ts`),
  so it applies only when the Mac window started shrunk; the full 890 pt window and Windows keep the
  current look. Tighter main padding/gap, smaller header (24 px logo, 16 px title), smaller group,
  row, select and button padding, 11 px hints. Measured in the mocked UI with a Mac user agent (new
  `?mac=1` in the preview copy's `mock.ts`): heights needed are now 600–647 pt with push-to-talk off and
  623–729 pt with it on (Turkish 606/667, Spanish 729), so everything fits a ~790 pt window with room to
  spare; an 11" Air (~656 pt window) may still scroll a little with push-to-talk on. Screenshot checked
  at 420×790 (Turkish, capturing a key). `npm run build` clean. Pushed as f566294; the owner's test DMG:
  everything fits except Spanish with push-to-talk on (Safari 14 renders a bit taller than Chromium;
  Turkish at 667 pt in Chromium fit on the Mac, Spanish at 729 did not).
- 23 Nov (owner): **Spanish with push-to-talk, tightened again** (not released). In Spanish the key
  capture button took four lines (long "Pulsa una tecla…" in a narrow monospace button next to a long
  caption) and the two sound test buttons wrapped. Now, in the compact Mac layout only: the push-to-talk
  caption sits above a full-width key button (`formgrid pttgrid` in `App.svelte`), whose "press a key…"
  text uses the UI font at 12 px (an assigned shortcut stays monospace); card/group/row/select padding
  and hint line height a little smaller. Spanish `test.tone` "Reproducir sonido de prueba" → "Probar
  sonido" (pairs with "Probar micrófono"; on every platform). Heights needed with push-to-talk on (mocked
  UI, Mac user agent): Spanish 729 → 647, Turkish 667 → 632, the tallest now Japanese 681 and Russian
  664, all below the 667 that fit on the Mac; push-to-talk off ≤ 608. Screenshot at 420×790 (Spanish,
  capturing). `npm run build`, `npm run check` clean. Not yet seen on the Mac.
- 23 Nov (owner: "push and release"): **v0.1.23 released.** Version bump commit 25fd78c, tag `v0.1.23`;
  fmt, clippy (workspace without the app, `-D warnings`) and `cargo test` clean before tagging. Release
  workflow green (windows, macos, linux-server); assets: installer + `.sig`, portable exe, universal DMG,
  macOS updater bundle + `.sig`, both servers; `latest.json` says 0.1.23 with Windows and macOS entries.
  Contents since v0.1.22: macOS `restart_app` after an in-app update, Windows header logo, window not
  resizable, errors shown to the user in the log, arrow cursor on the title bar buttons. No server or
  protocol change, so the Oracle server stays on v0.1.21 (not updated).
- 23 Nov (owner): **title bar buttons keep the arrow cursor.** The minimize/close buttons of the own Windows
  title bar inherited `cursor: pointer` from the global `button` rule; `.tb-btn` now sets `cursor: default`
  (`ui/src/app.css`), like native caption buttons. CSS only; not tried in the real app (the title bar shows
  only in the Tauri Windows build). Not released.
- 21 Nov: **CS2 session bera + kuzen in the Server Lobby, followed live** (logs only, no code change):
  server journal over SSH, bera's client log, kuzen's two uploads (16:23 and 17:07 UTC). Both v0.1.22,
  server v0.1.21. bera 16:13–17:07 UTC (54 min, RTT 40–44 ms, 0 lost); kuzen's last connection
  16:30–17:07 (38 min, RTT mostly 39–47 ms, spikes to 80 and 123 ms, 12 QUIC packets lost, cwnd down to 4116).
  - **kuzen's headset kept dropping out (hardware):** 16:25:45–16:26:59 Windows reported the device as
    removed seven times (`output error` / `input error: The requested device is no longer available`).
    The engine recovered every time (`audio pipeline recovered attempts=1…6`, `fallback=true` switched to
    another output at 44.1 kHz) and fell back to listen-only when the mic was gone. kuzen reported audio
    only in the right ear and no mic at first; both match a loose USB/jack connection. He restarted the app
    three times (all `bye`); from 16:29 on no device error. The first upload's
    `no microphone, listening only: input device: audio device not found` was the same cause.
  - **Possible gaps seen:** after the mic came back, the app stayed listen-only (16:27:20 until kuzen
    restarted the app at 16:28/16:29; not confirmed whether the engine re-probes the input at all);
    `find_device` with a saved name that no longer exists fails with `NoDevice` instead of falling back to
    the default device; the `audio pipeline started` line has no device names, so a fallback to another
    device is only visible through the rate change.
  - **bera hearing kuzen (~37 min, ~111k frames):** late 296, underflow 333 (~0.3%), lost 45, FEC 13. The
    jitter target jumped from 40 to 160–240 ms on a handful of late packets (16:31, 16:34–16:35) and fell back to 40
    within ~30 s, after which new late packets turned into underflows (e.g. +27 late / +27 underflow in
    30 s at target 40, +23/+25 later). Target distribution over the 30 s lines: 40 ms ×27, 60 ×19, 80 ×17,
    100 ×6, 140 ×2, 160 ×3, 240 ×1. The 123 ms RTT spike at 16:35:22 cost ~10 frames (FEC 0).
  - **kuzen hearing bera:** late 160, underflow 170, lost 37, FEC 2 by 17:07; worst 30 s at 17:04 (+30 late).
  - bera's own PC: one 96-sample `out_underrun` at 17:01:58 (2 ms, under CS2 load); no stalls.

- 19 Nov (owner): **the window is no longer resizable** (`resizable: false` on Windows and macOS; it was
  already not maximizable). `fit_window_to_screen` keeps shrinking it on short screens (`set_size` is not
  limited by `resizable`). Clippy (app) and `cargo test -p callout-app` clean; not tried in the real app.
- 19 Nov (owner: "add it"): **errors shown to the user now go to the log.** `engine.rs`: the `emit` helper
  and `open_preview` call the new `log_error`, which writes `error shown to the user kind=… detail="…"`
  (`warn`; `RoomClosed` at `info`); an invalid invite logs only its kind, since the detail echoes what the
  user typed. The messages hold no device names (`find_device` fails with `NoDevice`); user paths and the
  fingerprint are masked by the upload scrub anyway. Checked with the talk example: an unreachable server
  logs `kind=ConnectFailed detail="could not connect: connection lost: timed out"`, a bad invite
  `kind=InvalidInvite`. fmt, clippy (workspace without the app, `-D warnings`), `cargo test -p callout-core`
  clean. Not released.
- 19 Nov: **reviewed the new server logs** (logs only, no code change): six uploads (17 Nov bera + Furkan,
  18 Nov mamitest, macBera, 18 Nov evening bera + Furkan) and the server journal since 17 Nov.
  - **17 Nov, bera + Furkan (Canada, Mac, v0.1.20), room GX4ANAKQ 17:25–18:21 UTC (~56 min)**, both `bye`.
    Furkan restarted the app twice in the first minute (user quits, 3 connections), then stayed. His network
    was worse than on 16 Nov: RTT median 134 ms, p90 ~290, max 580 (server: median 137, p90 276, max 603).
    bera hearing Furkan: almost nothing concealed (lost 61, late 55, FEC 42 of ~168k frames) but the jitter
    target sat at **median 140 ms, p90 240 (the cap)**: the adaptive buffer bought quality with latency.
    Furkan hearing bera: lost 286, late 119, **FEC 1**, target median 40 ms; the server lost only 1 packet
    to him, so these are stall clumps trimmed by the resync (e.g. 17:59 lost 27, late 1), the pattern of 16 Nov.
  - **18 Nov evening, same pair, both v0.1.22, room GX4ANAKQ 21:12–22:22 UTC (~70 min)**, both `bye`.
    Furkan RTT median 124 ms, p90 151, max 523. bera hearing Furkan: lost 578, late 288, FEC 8 (~0.5% of
    frames), target median 80, p90 240; the losses again come as clumps (12 steps of 16–51 frames, e.g.
    22:19 lost 51 late 3), i.e. Furkan's uplink stalls. Furkan hearing bera: lost 490, late 562, **FEC 123**;
    most of it one real loss burst 21:16:23–21:17:03 (server → Furkan lost 365 packets in 40 s, ~18%, cwnd
    at its 2904-byte floor), where FEC recovered ~62%; target median 60, p90 120.
  - **First real macOS in-app update:** Furkan's Mac went 0.1.20 → 0.1.22 at 21:09 (27 s between the two
    starts, so a manual restart, as expected before `restart_app`); the microphone kept working
    (`has_input=true`, real noise floor). macBera (2014 MacBook) also went 0.1.20 → 0.1.22 on 18 Nov morning.
    mamitest (Windows) updated 0.1.21 → 0.1.22 in-app in 14 s.
  - **Lobby with three devices, 18 Nov 05:29–05:57 UTC** (bera, mamitest on Windows, macBera; up to
    members=3): clean, bera saw 0 lost from mamitest and 5 from macBera; joins/leaves/mute/deafen all on the
    server log; macBera quit twice while in the lobby, both `bye`.
  - **Found: errors shown to the user never reach the log.** mamitest's first connection after the update
    (05:26) closed after 0 s with `bye`; its log shows the audio thread starting three times (priority
    raised at :28, :33, :41) before `audio pipeline started`, with no warning in between. In `engine.rs`
    a failed `start_audio` sends only `Event::Error` and closes the client; `Event::Error` is never traced,
    so the reason (output device? default config?) is lost. The same hides failed connects: macBera's four
    v0.1.20 starts on 18 Nov 05:15–05:42 never logged `connected` and nothing else. Proposed fix: log every
    `Event::Error` (kind + message) with `tracing::warn!` in the engine (not in the cpal callback).
  - Minor: two single ~100 ms stalls on bera's Windows (17 Nov 17:25, capture and playback callbacks too,
    so the whole system; 18 Nov 05:39); start-up underruns of 1056–2048 samples as before; AEC ERLE ~0 on both
    sides (headphones). bera's VAD calibrated at -32 dB once (talking during calibration); the adaptive floor
    follows down fast, and bera sent in 60% of the stats windows that evening, so no harm.
  - Takeaway: Furkan's Wi-Fi stalls stay the main source of concealment and push bera's jitter target to its
    240 ms cap for long stretches; step (5) of the jitter plan (faster recovery after a spike) is the lever.
- 18 Nov: the owner tested v0.1.22 (in-app update from v0.1.21 brought the new window and icon; mute
  survives a restart; settings fit; no maximize; the Mac looks fine). Found: on the Mac the update stops
  at "100%" and needs a manual restart, because the macOS updater does not relaunch. Fix: new app command
  `restart_app` (`AppHandle::restart`), called by the UI after `update.install()`. svelte-check, UI build,
  clippy (app), `cargo test -p callout-app`, fmt clean; the macOS build itself is left to CI (the cross
  check from Windows needs a C toolchain). Then (owner): the Mac shows the icon left of "CallOut" in the
  header, Windows did not (hidden there because the title bar has a small one); now shown on Windows too
  (header height unchanged, settings still fit; checked in the mocked UI). The owner then asked to drop
  the small icon from the title bar (only minimize and close remain there, right-aligned). Not committed,
  not released.
- 18 Nov: **v0.1.22 released** (the redesign below), at the owner's request before the three "Redesign"
  manual tests (asked once; owner: release now). Local checks before the tag: fmt, clippy (workspace and
  app), `cargo test` (all green), svelte-check, UI build. Tag v0.1.22 on c1f5e8a; Release run 44242886541
  green (windows, macos, linux-server), CI on main green; assets: installer + `.sig`, portable exe,
  `CallOut_0.1.22_universal.dmg`, `CallOut_universal.app.tar.gz` + `.sig`, both servers; `latest.json`
  0.1.22 with windows-x86_64, darwin-aarch64, darwin-x86_64, all signed. Release notes got a "What's new".
  No server update (no server/protocol change).
- 18 Nov: **redesign** (owner's request, after previews outside the repo). New icon: plum rounded square
  (`#4a3468` → `#34244a`), honey (`#e8ba46`) headset with a mic boom, thin dark outline (`#14101a`), a bolder
  drawing for 16–48 px; all seven files in `apps/callout-app/icons/` replaced, `ui/public/logo*.png` for the
  UI. UI (`ui/src/app.css`, `App.svelte`, `Icon.svelte`): theme from the icon (honey primary buttons with the
  dark outline, plum cards), header with the logo, connect screen with the big icon, Server Lobby card in the
  icon's colours, inline create/join fields, recent rooms with initials, members with initial avatars and a
  honey speaking ring, a bottom dock with round mic/deafen/leave buttons, settings in groups with switches.
  **Windows title bar:** `tauri.windows.conf.json` (window without decorations), new window permissions in
  `capabilities/default.json`, the bar in `App.svelte` (drag region, minimize / close → CloseRequested →
  tray as before); macOS keeps its own bar. Then (owner, after trying it): the settings page scrolled, and
  no full screen wanted: the settings card is more compact (smaller paddings, footer hidden there), the
  default height is 920 on Windows / 890 on macOS (was 780; measured so every language fits with
  push-to-talk on), and the window is `maximizable: false` everywhere (maximize button and its i18n keys
  removed; `allow-internal-toggle-maximize` stays so the drag region's double click does not error, Tauri
  checks `is_maximizable` there). **Per-player
  mute:** new `muted_users` (nicknames) in `Settings`, owned by the app like `user_volumes`; new command
  `set_user_muted` sends gain 0 or the saved volume straight to the engine (so 0 is never saved as the
  volume); `saved_volumes_for` sends 0 for muted players on every room update. New i18n keys in all 11
  dictionaries (`window.*`, `room.muteUser`/`unmuteUser`); the mic test hint now says yellow, not green.
  Checks: svelte-check, `npm run build`, clippy (incl. the app), `cargo test -p callout-app` (8 tests, one
  new), fmt. Checked in the browser with a mocked Tauri layer (title bar, mute commands, Turkish/Arabic);
  **not run as the real app yet**, nothing committed.
- 18 Nov: **v0.1.21 released** (Server Lobby + the 24 kHz Bluetooth warning). The first CI run on the lobby
  commit failed on Linux: a race in the new e2e test (A's leave and B's join on two connections; the server
  took B's join first), fixed in a376edf (A leaves and rejoins on its own connection), CI green. Tag
  v0.1.21 on a376edf; Release run 44218274149 green (windows, macos, linux-server); assets: installer +
  `.sig`, portable exe, `CallOut_0.1.21_universal.dmg`, `CallOut_universal.app.tar.gz` + `.sig`, both servers,
  `latest.json` (0.1.21; windows-x86_64, darwin-aarch64, darwin-x86_64). Release notes got a "What's new".
  Oracle server updated at 02:05 UTC (nobody connected): protocol 2, 12 rooms loaded, invite link unchanged;
  the talk example joined the live lobby. Ubuntu auto-updates are on; a kernel/`libc6` reboot pending since
  13 Nov was done at 02:10 UTC with the owner's OK (nobody connected, back in ~60 s on kernel
  7.0.0-1016-oracle, service active, 12 rooms, firewall rule kept, live lobby joined again).
  Then the owner (Windows): v0.1.20 on the new server used rooms by code with no lobby card; the in-app
  update to v0.1.21 worked and the lobby card showed, join/leave fine ("Süper!"). Owner asked in passing about Oracle's Nov 2026 Critical Security Patch Update:
  it covers Oracle software products only (Database, MySQL, Java, WebLogic, VirtualBox…), nothing on the
  server; OCI compute and Ubuntu are not in it.
- 18 Nov: **Server Lobby** (owner's request: a fixed public room on every server, open to everyone with the
  invite link, no password, no auto-join, **not joinable by code**, only shown in the UI; costs nothing while
  empty). Owner chose: no auto-join, no code.
  - Protocol 2 (`callout-proto`): `ClientMsg::JoinLobby` appended, `PROTOCOL_VERSION` 2, new
    `MIN_PROTOCOL_VERSION` 1 (the server accepts 1–2, so v0.1.20 apps keep working), `LOBBY_CODE` "LOBBY"
    (the lobby's `RoomInfo.code`; `JoinRoom` never finds it: generated codes have 8 characters and no `O`).
  - Server: `Registry` holds the lobby next to the code map (name "Server Lobby", no owner → `CloseRoom`
    gives `NotOwner`; not in `rooms.tsv`, never expires; speaking ticks include it). Handshake refusals now
    go through `refuse()`, which waits up to 3 s for the client to hang up: before, the server dropped the
    connection right after `VersionMismatch` and the client mostly saw "connection lost".
  - Client: `Client::connect` tries protocol 2; if the connection ends between `Hello` and `Welcome`
    (`VersionMismatch` or a lost stream, which is what v0.1.16 servers produce) it reconnects once with
    protocol 1. `Client::has_lobby`, `join_lobby`; engine `Command::JoinLobby`, `Event::Connected { lobby }`,
    reconnect rejoins the lobby with `JoinLobby`; app snapshot carries `lobby`; talk example `--lobby`.
  - UI: a "Server Lobby" card on top of the home screen when the server has one (translated name in all 11
    languages, `serverLobby.*`); in the lobby no code button, no close; never added to recent rooms.
    Owner's colours (second try; the bright `#8634eb` box with a yellow title did not please): dark purple
    `#2a2140` background, `#8634eb` frame, lilac `#c9a7ff` title, purple `#8634eb` "Join" button (hover
    `#9a4ff0`) instead of the blue primary. Seen in the dev app on a local server: the
    card shows with the new build and protocol 2; the new build on the Oracle server (v0.1.16) fell back to
    protocol 1 and showed no card, as intended.
  - Checks: new tests (registry: lobby not saved/not joinable by code; e2e: two clients in the lobby hear
    each other, leave and rejoin, protocol 1 client served, unknown version refused; engine: lobby join/
    leave twice, `LOBBY` by code → RoomNotFound, close → NotOwner). `cargo test` for the touched crates and
    clippy (`-D warnings`) clean, `cargo check -p callout-app`, svelte-check 0 errors, UI build fine.
    Live with the talk example: against a local **v0.1.16** server it fell back to protocol 1
    (`lobby=false`, "this server has no lobby"); against the new server it entered "Server Lobby" and wrote
    no `rooms.tsv` entry. Not seen in the app window yet (see Pending manual tests). One unrelated flake:
    `engine_reconnects_after_server_restart_and_rejoins_its_room` once failed because its server did not
    print an invite link (port bind); three reruns passed.
- 16 Nov: **first long-distance field test** (logs only, no code change): "Furkan" in Canada (macOS,
  v0.1.20, ISP Bell) + Windows "bera" in Turkey, room GX4ANAKQ 17:29–19:16 UTC (~107 min), both sent logs,
  both left with `bye`. Owner: Furkan's voice clean. RTT Furkan ↔ Frankfurt **118 ms** median (bera 38 ms),
  with four short spikes to 600–930 ms on Furkan's side (18:31, 18:56, 19:15; server saw 907 ms once).
  - **Server → Furkan (Furkan hears bera):** server `lost_packets` 391 of ~84k datagrams (0.5%), but almost
    all in the first 3 minutes (~5–10% per 10 s at 17:29–17:32, quinn's cwnd sat at its 2904-byte floor),
    then near zero. Furkan's buffer: lost 372, **FEC recovered 237 (64%)**, late 479, underflow 511;
    target median 40 ms (max 200), wait median 31 ms. Concealed ~1% of frames, mostly in that first burst.
  - **Furkan → bera (bera hears Furkan):** lost 609 but **FEC only 2**, late 751, underflow 763; target
    median 60 ms (p90 80, max 240), wait median 51 ms. The losses are not random drops but stalls: the
    stream goes quiet, a clump arrives and the resync trims it (big `lost` steps with few `late`, e.g.
    18:17 lost 47, 18:57 lost 56 with target 240), so there is never a next packet to take FEC from.
    Fits the RTT spikes: Furkan's uplink stalls for ~0.5–1 s now and then (he was on Wi-Fi; on a Mac
    possibly AWDL / background scans). ~2% of frames concealed overall; the owner did not notice.
  - Furkan's Mac: `boost="qos user interactive"`, `loop_max_ms` ≤ 14, no loop stalls, output 44.1 kHz
    (resampled), reserve 20 ms (`out_fill_ms` 28), one start-up underrun (1536 samples) only; AEC ERLE ~0
    (headphones). bera: `loop_max_ms` ≤ 5, a few 96–576-sample underruns each followed by the reserve
    stepping 20 → 30 → 20 ms.
  - Takeaway: 118 ms of distance is fine for voice; the weak spot is the sender's Wi-Fi stalls, which
    neither FEC nor the jitter buffer can hide (only a bigger buffer, at a latency cost). Confirmed: Furkan
    was on Wi-Fi.
- 14 Nov: **macOS field test with v0.1.20** (logs only, no code change): "cakal" on a second Mac with
  AirPods + Windows "bera", same home network, room 8LLBVGX7 15:21–15:27 UTC, both sent logs; the owner says
  the voices were clean. Server: no loss, both left with `bye`. RTT bera 37 ms steady, cakal 44–62 ms; the
  jitter target went to 100 ms both ways (the Mac's link, probably Wi-Fi), `late` 42–52 per side (<1% of
  frames), `lost` 2–4 (on the home network, the server saw none). Mac: `boost="qos user interactive"`,
  `loop_max_ms` 5–9, no stalls, input **24 kHz** (AirPods hands-free; the Bluetooth warning only fires at
  ≤16 kHz), output 48 kHz at first; at 15:22:49 and 15:22:57 the device changed and the pipeline
  `recovered attempts=1/2 fallback=true` with output now 24 kHz (macOS moving output to the AirPods): the
  first real-world sign that device recovery works on macOS. One start-up underrun (6656 samples, ~0.14 s)
  before the room, none after. AEC showed ERLE ~0 on both sides (headphones, nothing to cancel).
  Then (owner's choice): the Bluetooth mic warning's limit went from 16 to 24 kHz (`App.svelte`, comment
  in `callout-core` `AudioStarted`); svelte-check clean, not seen in the app yet (the browser preview has
  no engine to report an input rate).
- 11 Nov: **v0.1.20 released** (the macOS app; app only, server stays on v0.1.16). PR #1 (`macos`)
  fast-forwarded into `main`; Release run 43077641548 green (windows, macos, linux-server); assets: Windows
  installer + `.sig`, portable exe, `CallOut_0.1.20_universal.dmg`, `CallOut_universal.app.tar.gz` + `.sig`,
  both servers, `latest.json` (served at releases/latest: version 0.1.20, Windows + darwin entries).
  Release notes got a "What's new" (CallOut for Mac; nothing new on Windows). Before the release, on the
  owner's MacBook with CI test DMGs: blank window fixed (`safari14` UI target), quit now leaves the server
  (`RunEvent::Exit`), Dock icon on Apple's grid (`icon.icns`), flat drop-downs on macOS; the test DMG no
  longer opens the Web Inspector. CI: Linux apt falls back to the kernel.org mirror (the runners could not
  resolve *.ubuntu.com for an hour), Mac jobs do not wait for Linux. `main` has a ruleset (no deletion, no
  force push). Owner: sound clean, AGC stays off by default; listening test and mouse side button with the
  release.

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
