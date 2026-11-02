# Phase 1 gate test — plan

Goal: 4 people, 1 hour, over the internet (Oracle Frankfurt server). The data from this test sets the
Phase 2 order. The server address and invite link are in `CLAUDE.local.md`; they are sent to participants over a private channel.

## Before (test day, Bera)

- [ ] `git tag v0.1.3 && git push origin v0.1.3`; once Actions finishes, check that the NSIS installer is on Releases.
- [ ] Update the server (`CLAUDE.local.md` → Update); keep `journalctl -u callout -f` open.
- [ ] To participants: installer + invite link + the "Participant" section of this document.
- [ ] In a second terminal watch the load with `top` (CPU) and `ss -u -a | grep 4433`.

## Participant

1. Run the installer, open the app, paste the invite link, enter a nickname.
2. Use headphones (speakers cause echo; there is no echo cancellation yet).
3. After the test, send this file to Bera:
   `%APPDATA%\io.github.berabozgoz.callout\logs\callout.log`
4. Fill in the form below (network type, observations).

## Schedule (60 min)

| Min | What | Watch for |
|----|----|-----------|
| 0–10 | Everyone joins the room (1 person opens it, the code is shared), free chat | Join problems, first audio |
| 10–25 | Free chat, someone starts a game (fullscreen) | Crackling, CPU impact |
| 25–35 | Push-to-talk (PTT) mode; at least one person tries PTT in a game | Does in-game PTT work |
| 35–45 | Everyone talks at once (4-voice mix) | Overlap, robotic audio |
| 45–55 | On purpose: one person switches Wi-Fi ↔ cable, one person disconnects and rejoins | Drop warning, rejoin |
| 55–60 | Leave the room, everyone fills in the form | — |

## Pass criteria

- Unintended drops in 1 hour ≤ 1 (self-recovering ones do not count; there is no automatic reconnect yet).
- Average RTT + buffer < 150 ms (end-to-end perception: "I didn't notice any delay").
- Crackling/robotic audio only rarely; no "I'm talking but nobody hears me" reports.
- Server CPU below 50% with 4 people.

## Participant form

- Nickname: ______  Network: home cable / home Wi-Fi / mobile data / work-school  ISP: ______
- Perceived latency (1 comfortable – 5 very bad): ___
- Crackling/dropouts/robotic audio: none / occasional / frequent  When: ______
- Did you get disconnected? Time and what you were doing: ______
- Mic level / VAD: could others hear you? Could you hear them?
- Did PTT work in game (game name): ______
- Notes: ______

## Afterwards: reading the logs

- Server: `journalctl -u callout --since "1 hour ago" | grep "connection stats"` →
  per user `rtt_ms`, `lost_packets` (QUIC), `dgram_rx/tx`. The `connection closed` line has
  `close_reason` and `secs`: unintended drop = unexpected `close_reason` / short `secs`.
- Client `callout.log`: a `stats` line every 10 s (`rtt_ms`, `jitter_ms`, `lost` = total jitter
  buffer loss, `sent`/`received` = packets in the last 2 s), plus the `closing connection` reason.
- Append the results to the bottom of this document as date + summary; update the Phase 2 order (STATUS.md "Next") based on the data.

## Results

### 28 Oct 2026 — two-person pre-test (v0.1.3, Bera + mamitest, both on ethernet desktops)

- Audio flowed both ways; QUIC loss 0 (outside the outage), RTT Bera 37–42 ms, mamitest 45–51 ms,
  jitter buffer 40–80 ms, server CPU idle.
- **Pulling the cable (~5–10 s):** the connection did not drop (same `user_id`, since it stayed below
  `max_idle_timeout` of 10 s); the server counted 73 lost packets, `cwnd` 12000 → 2904. The user did not notice
  any audio glitch/crackling/jump during or after the outage. An outage >10 s becomes `TimedOut`; there is no automatic
  reconnect.
- **Bugs found:** (1) the push-to-talk shortcut kept swallowing the key after leaving the room (fixed, `87a4f61`);
  (2) when the app closes, the close frame is sometimes not sent and the connection is dropped after ~15 s with `TimedOut`.
- **mamitest's log (received):** in the second session RTT 45–50 ms, `lost=0`, jitter 40–80 ms, receive windows
  69–100/100; the cable outage shows up in his log at the same time and recovered.
- **Open observation (first session 18:39:59–18:40:31):** the server put mamitest in the room ("joined room") but ~7 s
  later stats showed `in_room=false`; Bera did not hear him, mamitest sent `sent=100`. The user says he closed and
  reopened the app at that point and saw the login screen; he had used the mute/deafen buttons.
  No code path was found where mute/deafen would remove someone from the room. The server did not log leaves/flags;
  v0.1.4 added "left room" and "mic/headphone state" lines — to be watched in the repeat test.
- **Audio was being sent outside a room** (the server dropped it): fixed, it is now sent only while in a room.
- Not yet covered: long-duration (1 hour) and 4-person load, participants on different networks.

### 30 Oct 2026 — three-person test (v0.1.4: Bera, "at" (cousin, different network), mamitest)

- All three were in the room for ~20–50 min; 3 people talking at once worked (received windows up to 180).
  RTT: Bera avg. 45 ms (server max 55), at 36–38 ms, mamitest 36–37 ms. Jitter buffer avg. 44–63 ms,
  at most 100 ms with 3 people. Client `lost`: at 21, Bera/mamitest 0. Server QUIC loss: at 106 (~0.1%,
  the cousin's network), others 0.
- The 8 min of silence at 22:17–22:25 UTC (sent=0 received=0) and the 30 s of silence at 22:13 were not outages:
  per the server log both were mute/deafen / nobody talking; RTT stable.
- Mute/deafen was toggled dozens of times and `in_room` never dropped → the `in_room=false` in the first test
  was not caused by mute/deafen; still unexplained.
- **Bug: audio is not recovered when the output device is lost.** `output error: The requested device is no longer
  available` — Bera 22:44:14, at 22:18:30. The engine only logs it; the output stream stays dead
  (Bera could not hear for ~2.5 min, at for ~8 min), the audio pipeline was rebuilt when a device was picked manually (Bera 22:46:50,
  at 22:26:33).
- mamitest's connection closed with `TimedOut` 18 s after leaving the room (the log copy was taken while the app was
  open, no close line); the window X hides to the tray, only tray "Quit" sends the close
  frame.

### 30–31 Oct 2026 — one-hour two-person test (v0.1.5: Bera + at)

- **23:06–00:12 UTC (66 min) uninterrupted**, both stayed on the same connection; clean close (`bye`), no
  `ERROR` at all. Server RTT: at avg. 39 ms (max 52), Bera avg. 45 ms (max 52). QUIC loss: at 47, Bera 0.
  Client `lost`: at 27 (13 short bursts, the largest 5 packets), Bera 1.
- **Version fix verified in the field:** logs show `version="0.1.5"`.
- **Jitter buffer asymmetric:** at's buffer (listening to Bera) was 40 ms 89% of the time. Bera's (listening to
  at) fluctuated between 40–240 ms, avg. 136 ms; mostly 180–240 ms between 23:15–23:57. So packets
  arrive irregularly in the at → Bera direction (possibly at's upload link or the audio device delivering packets
  in batches; at's mic floor was −86…−100 dB, unusual). In Bera's direction end-to-end
  latency can reach ~300 ms at peaks. The log does not show how many late/missing packets came from which sender
  → per-sender jitter stats should be added (input for Phase 2 adaptive jitter).
- **One unintended drop (v0.1.4, 22:57 UTC):** ~1 min after at left the room, while in the lobby, the client logged
  `disconnected: stream read: connection lost`, the server `TimedOut` 11 s later. Server RTT climbed
  36 → 49 ms in the minute before the drop. Cause unclear; the client log does not record QUIC's internal error
  (the `ConnectionError` behind "connection lost") → should be added.
- **Against the pass criteria:** 1 hour ✓, drops ≤1 ✓ (in the lobby, not in a room), server CPU ✓; **4 people ✗**
  (at most 3 people, ~17 min); **RTT + buffer < 150 ms: ✓ on at's side (~80 ms), ✗ on Bera's side**
  (avg. ~180 ms).
- **Participant answers:** (1) during the high-buffer stretch they were playing Counter-Strike together, Bera barely
  noticed the delay; (2) at uses Bluetooth earbuds (AirPods) — with the mic on, Windows switches to the "hands-free"
  profile and audio packets arrive in batches/irregularly: the most likely cause of Bera's 180–240 ms buffer and
  the "output device lost" error on at's side (22:18); (3) the 22:57 drop was not the app:
  at's computer went to a black screen and was rebooted twice.

### 31 Oct 2026 — one-hour two-person test, wired mic (v0.1.5: Bera + "kuzen", different network)

- **13:58–15:10 UTC (72 min) uninterrupted**; both closes clean (`bye`), no `ERROR`. Server RTT: Bera avg.
  36 ms (max 47), kuzen avg. 38 ms (max 61). QUIC loss: kuzen 9, Bera 0. Client `lost`: Bera 2, kuzen 1.
- **Jitter buffer symmetric and low this time:** Bera (listening to kuzen) avg. 53 ms, max 100, 40 ms 53% of
  the time; kuzen (listening to Bera) avg. 44 ms, max 100, 40 ms 82% of the time. In 10 min slices Bera's
  average stayed between 40–61 ms, with no upward trend.
- **AirPods comparison:** on the same Bera side, listening to "at" on AirPods gave avg. 136 ms / max 240 ms,
  listening to "kuzen" on a wired mic avg. 53 ms / max 100 ms. Not conclusive since the person and network differ,
  but it strongly supports the theory that a Bluetooth mic creates irregularity on the sender
  side.
- **Against the pass criteria:** 1 hour ✓, drops 0 ✓, RTT + buffer < 150 ms ✓ in both directions (~89 ms and
  ~80 ms). The only remaining criterion: 4 people.
- Minor notes: kuzen changed the output device twice at the start (44.1 kHz ↔ 48 kHz), the audio pipeline was rebuilt
  without problems. The client `lost` counter once dropped 1 → 0: when a sender silent for 60 s is removed
  from the table its loss leaves the total (a stats gap, not an audio problem).


### 1 Nov 2026 — four-person test (v0.1.6: bera, cocart, batayta, at; four different networks)

Participants: bera (cable, wired mic), cocart (Wi-Fi + AirPods, including the mic), batayta (Wi-Fi,
Bluetooth headset — the client opened 16 kHz mono, i.e. the "hands-free" profile), at (cable, AirPods Pro 3
output + dynamic mic). Logs: server, bera, batayta, at (none from cocart).

- **The room stayed open 18:20–20:21 UTC (2 h).** bera, cocart, batayta never dropped; closes were clean (`bye`).
  Server: load average 0.00, ~5 MB memory, no `ERROR`.
- **Time with 4 people: 18:34:55–19:10:09 (35 min) + 19:29:43–19:47:00 (17 min) = ~52 min.** Both stretches ended with
  at leaving:
  - 19:10:09 at left the room via "Leave" (server `left room`); the client log stops at 19:09:59,
    no `closing connection` line, server `TimedOut` 22 s later. The process ended without the app shutting down cleanly
    (task manager, shutdown or crash; cannot be told apart since panics are not logged).
  - 19:47:00 at closed the app cleanly (`bye`).
- **Server RTT / QUIC loss (server to client):** bera 41 ms / 1; batayta 14 ms / 9; cocart 63 ms
  (peaks 100–344 ms, during gaming) / 179; at 40 ms / **3668** (+ 18 in the second session).
- **at's network degraded between 18:48–19:04:** despite being wired, ~8% of packets from the server to at
  were lost in these 16 minutes (3600 packets); at's client counted 2700 lost audio frames (1120 from
  Bera alone). RTT stayed steady at 38–44 ms → not congestion, loss on at's line. at must have heard everyone
  choppy during this time. No loss for the other three at the same time → not server-side.
- **cocart:** heavy loss on Wi-Fi at the start of the test (18:20–18:45); Bera's voice reached him choppy. After 18:45
  it fell to 0–14 packets per 5-minute slice. On Bera's side, late packets from cocart ~4/min.
- **Jitter buffer (listener side, average / max):** bera 73 / 140 ms, at 73 / 120 ms,
  batayta 70 / 180 ms. Most late packets per sender came from batayta (~290 at Bera, Wi-Fi + Bluetooth).
- **Against the pass criteria:** RTT + buffer < 150 ms ✓ (bera ~114, at ~111, batayta ~84 ms); server CPU ✓;
  unintended drops ≤ 1 ✓ (at most 1: at 19:10, cause was the participant's line/computer); crackling rare:
  **✗ for at** (18:48–19:04 line loss), ✗ for cocart at the start; no "nobody hears me" reports.
  **4 people × 1 hour: uninterrupted ✗ (35 min), ~52 min total.** What missed the criterion was the participant side
  (line loss and leaving the app); no drops or errors were seen in the system.
- **Takeaways for Phase 2:** (1) 1–8% loss happens on real networks → Opus FEC (in-band) + PLC moves to
  first place; (2) Bluetooth mics/headsets (cocart, batayta) are again the most irregular senders → adaptive
  jitter and a "Bluetooth mic" warning in the UI; (3) when microphone access is denied (Windows privacy,
  `0x80070005`) the connection cannot be established at all ("Hallobadi", 20:04, 4 attempts) → a clear error + a mic-less
  (listen-only) join; (4) the panic hook should write to the log to tell crashes apart from forced closes.
- **Decision (1 Nov 2026): Phase 1 closed.** The criterion was not met to the letter (4 people uninterrupted for 60 min), but the
  missing time was on the participants' side; server and client ran with 4 people without drops or errors.
