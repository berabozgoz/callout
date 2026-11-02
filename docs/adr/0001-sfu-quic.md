# ADR 0001 — SFU topology and QUIC transport

Date: 2026-10-26 · Status: accepted

## Context
Low-latency voice chat for gamers, 5–10 people per room. P2P means N-1 uploads and NAT traversal
problems; the WebRTC stack is needlessly complex for a non-browser client.

## Decision
Our own Rust SFU; a single QUIC connection (quinn) between client and server. Control messages go
over a reliable stream, Opus packets over datagrams. The server does not decode Opus.

## Consequences
+ Low server CPU, easy to self-host, end-to-end encryption can be added later.
− Networks that block UDP need a fallback mode over a reliable stream (Phase 2).
− A browser client would need a separate WebRTC bridge.
