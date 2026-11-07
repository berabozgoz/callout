# CallOut — Architecture and roadmap (summary)

Full living document (diagrams, rationale, comments):
https://claude.ai/artifact/Fm7ZdEpMeUJSY3tpw36Faq — last decision date 26 Oct 2026.
This file is the summary kept in the repo; for protocol details **the code (`crates/callout-proto`) is the source of truth.**

## Product

Players meet with a room code and talk with under 150 ms end-to-end latency.
MVP is Windows only. Target room size 5–10 people (hard limit 16).

## Key decisions

| Decision | Choice | Rationale |
| --- | --- | --- |
| Topology | Server-based SFU | Client uploads 1 stream, downloads N-1; NAT problems are solved at the server |
| Transport | QUIC (quinn + rustls) | One connection: reliable control stream + unreliable audio datagrams; TLS 1.3 built in |
| Codec | Opus 48 kHz, 20 ms, 24–32 kbps, FEC + DTX | Transparent for speech, loss-resilient |
| Audio I/O | cpal (WASAPI) | Pure Rust; exclusive mode via the `windows` crate if needed |
| UI | Tauri 2 + Svelte 5 | Audio and networking stay in Rust; the UI gets fast iteration |
| Identity | Ed25519 device key + nickname | No passwords; email accounts in Phase 3 |
| Hosting | Self-host friendly | Same binary; self-signed certificate + fingerprint; `callout://IP:port#fingerprint` invite |
| License | MIT OR Apache-2.0 | No GPL crates |

## System

```
Client A (Tauri)             callout-server (VPS / home PC)       Client B…N
┌──────────────────┐         ┌────────────────────────┐          ┌──────────────┐
│ Svelte UI        │         │ SFU: copies Opus to    │          │ Svelte       │
│ callout-core     │ ─audio─▶│ room members undecoded │ ─audio─▶ │ callout-core │
│ callout-audio    │ ◀audio─ │ rooms · auth · (chat)  │ ◀audio─  │ callout-audio│
│ callout-net/QUIC │ ◀control▶ SQLite                │ ◀control▶│ callout-net  │
└──────────────────┘         └────────────────────────┘          └──────────────┘
   audio = QUIC datagram (unreliable, 20 ms)   control = QUIC stream (reliable)
```

## Client

- `callout-proto` types · `callout-audio` cpal + Opus + jitter + mixer ·
  `callout-net` QUIC · `callout-core` state machine · `callout-app` Tauri shell · `ui/` Svelte ·
  `callout-input` push-to-talk listener · `callout-priority` audio thread priority (the two crates with `unsafe`).
- Send: mic → cpal callback (only a copy into the ring buffer) → audio thread:
  resample to 48 kHz, 20 ms frames, VAD, [Phase 2] noise suppression, Opus → datagram.
- Receive: datagram → per-sender jitter buffer (60 ms initial, adaptive) → Opus decode + PLC →
  per-user gain → mixer (f32 sum + soft limiter) → cpal playback. Join/leave cues (`callout-audio::cues`,
  synthesized, no sound files) and the test sound are added to the mix in the audio thread.
- The engine's audio thread joins the MMCSS "Pro Audio" task (fallback: `THREAD_PRIORITY_HIGHEST`); cpal
  already raises its own callback threads. Under a game's CPU load a normal-priority thread was run late
  (playback underruns, packets that looked late). Every 30 s it logs `audio timing` (largest gaps of the
  loop and both callbacks) next to the dropout counters.
- Tauri boundary: the UI sends `Command`, receives `Event`; audio data never crosses into the webview.

## Server

Single tokio process, single UDP port. Modules: `sfu` (one task per room, stamps with `stamp_sender` and forwards;
packets from muted members are not forwarded), `rooms`, `auth` (challenge → Ed25519 signature), `store` (sqlx/SQLite),
`admin` (health + metrics). Phase 3: `presence`, `chat`. Rooms are pinned to a server; no cross-server routing.

## Protocol

Control: `[u32 LE length][postcard(ClientMsg|ServerMsg)]`, rejected on `PROTOCOL_VERSION` mismatch.
Handshake: `Hello` → `Welcome{challenge}` → `Auth{signature}` → `Authed`.
Diagnostic log upload (user consent each time): a second bidirectional stream after authentication,
`[u32 LE length][postcard(LogUpload)]` + log text, answered by one `LogUploadReply`. Control messages are
untouched: an older server never accepts the stream (the client gives up after 20 s), older clients never open one.
Audio packet: `sender_id(4) seq(2) timestamp(4) flags(1) opus(≤400)`; ~52 kbps/speaker, 0 during silence with DTX.
Encryption: in the MVP, QUIC/TLS transport encryption; the `flags::E2E` bit is reserved for end-to-end.

## Roadmap

| Phase | Scope | Gate |
| --- | --- | --- |
| 0 Skeleton (1 week) | workspace, proto, CI, empty Tauri shell, device list | CI green; app lists devices |
| 1 Audio MVP (4–6 weeks) | loopback → Opus → QUIC two clients → jitter/mixer → rooms + self-host address → UI | 4 people for 1 hour; latency ≤ 150 ms |
| 2 Audio quality (3 weeks) | nnnoiseless, AGC, FEC, adaptive jitter, per-user volume, reconnect | intelligible at 5% loss + 100 ms jitter; keyboard inaudible |
| 3 Social layer (4 weeks) | accounts, friends, presence, text, persistent rooms | add friend → see online → invite to room |
| 4 Release (3 weeks) | signed NSIS, auto-update, crash reporting, closed beta | 20 users for 2 weeks; crashes < 1% |

Later: Linux/macOS, in-game overlay, end-to-end encryption, browser bridge, regional servers,
one-click self-host setup tool.

## Risks

cpal/WASAPI crackling (measure early with loopback) · UDP-blocked networks (fallback mode over a reliable stream) ·
single-region latency (self-host + multiple servers later) · SmartScreen (Phase 4 signing) · scope creep (gate criteria).

## Architecture decision records (ADR)

A new architecture decision is added as `docs/adr/NNNN-title.md`: context, decision, consequences.
