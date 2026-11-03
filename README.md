# CallOut

Low-latency voice chat for gamers: a small Windows app and a server you can host yourself.

- **Low latency:** Opus at 48 kHz in 20 ms frames over QUIC, an adaptive jitter buffer that follows
  your network, and a server that only forwards packets (it never decodes audio).
- **Clean voice:** RNNoise noise suppression (fans, hum, room noise), voice activity detection with
  an adjustable sensitivity, or push-to-talk with a global shortcut.
- **Your own server:** one binary and one UDP port; friends join with a `callout://` invite link.
- **11 languages:** English, Deutsch, Español, Français, Português (Brasil), Русский, Türkçe, العربية,
  中文（简体）, 日本語, 한국어.

Status: **Phase 2 (audio quality) in progress.** Rooms for 5–10 people, Windows only. See
[`docs/STATUS.md`](docs/STATUS.md) for where things stand.

## For players

1. Download `CallOut_<version>_x64-setup.exe` (installer) or `callout-app.exe` (portable) from the
   [Releases](https://github.com/berabozgoz/callout/releases) page. The app is not code-signed yet, so
   Windows SmartScreen shows "unknown publisher": choose **More info → Run anyway**.
2. Get an invite link (`callout://…`) from whoever runs the server, enter a nickname and connect.
3. Create a room and share its code, or join a room with a code. Rooms are permanent: the same code
   works next time (rooms unused for 30 days are removed), and the lobby lists your recent rooms.

Before your first session, open **Settings (⚙)**:

- Pick your microphone and speaker, then run the **sound test**: speak for 5 seconds and hear yourself
  the way others will. The line on the level meter is the voice activity threshold.
- Leave **noise suppression** on unless you have a studio-quiet mic.
- Choose **push-to-talk** and a key if you prefer it to voice activity. The shortcut is active only while
  you are in a room; F9 or a combination like Ctrl+Alt+… is safer than a plain letter.

If the connection drops, CallOut reconnects on its own and puts you back in your room.

### Troubleshooting

- **"Windows is blocking microphone access":** Settings → Privacy → Microphone → turn on "Let desktop
  apps access your microphone", then restart CallOut. Until then you can still join and listen.
- **Bluetooth headset warning:** when a Bluetooth headset's microphone is in use, Windows switches it to
  a low-quality hands-free mode. A wired or USB mic sounds much better; keep the headset for listening.
- **Choppy or late audio on Wi-Fi:** use a cable or 5 GHz Wi-Fi if you can.
- **Logs:** `%APPDATA%\io.github.berabozgoz.callout\logs\callout.log`. Quit from the tray menu before
  sending it to someone.

## For server hosts

The easiest option is a small Linux VPS (Ubuntu 24.04; the smallest plan is enough). As root:

```sh
curl -fsSL https://raw.githubusercontent.com/berabozgoz/callout/main/deploy/install.sh | sudo bash
```

The script downloads the latest release, installs a `callout` systemd service, opens **4433/udp** in the
local firewall and prints the invite link. Also allow UDP 4433 in your provider's firewall / security
group. Run the same command again to update.

- Logs: `journalctl -u callout -f` (connection stats every 10 s, joins/leaves, close reasons).
- Data: `/var/lib/callout` holds the server certificate and `rooms.tsv` (the room list). Deleting the
  certificate changes the fingerprint, and everyone needs a new invite link.
- Windows: run `callout-server.exe` from the Releases page; it prints the invite link.
- From home: forward **UDP 4433** on the router and set `CALLOUT_PUBLIC_IP=<your external IP>`. This
  does not work behind CGNAT (the router's WAN address differs from your public IP); use a VPS then.
- Environment variables: `CALLOUT_PORT` (4433), `CALLOUT_DATA` (data directory), `CALLOUT_PUBLIC_IP`,
  `RUST_LOG`.

## For developers

Requirements: Rust (stable), `cargo install tauri-cli --version "^2"`, Node.js 22+, and on Windows the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On Linux (libraries and server only):
`libasound2-dev`.

```sh
cargo test                                      # library tests (incl. network and jitter simulations)
cargo run -p callout-audio --example loopback   # mic → headphones local loop
cargo run -p callout-server                     # local server; prints the invite link
cargo run -p callout-core --example talk -- "<invite>" Name --create     # headless client: open a room
cargo run -p callout-core --example talk -- "<invite>" Name --room CODE  # headless client: join a room
cd ui && npm install && cd ../apps/callout-app && cargo tauri dev        # desktop app
```

Releases are built by GitHub Actions from a version tag (`git tag v0.1.12 && git push origin v0.1.12`);
the tag is the single source of the version number.

UI text lives in `ui/src/i18n/` (English is the source); adding a language is one dictionary file.
Corrections from native speakers are very welcome.

Architecture and roadmap: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). Development rules:
[`CLAUDE.md`](CLAUDE.md). Field test reports: [`docs/GATE_TEST.md`](docs/GATE_TEST.md).

## License

MIT or Apache-2.0, at your option (`LICENSE-MIT`, `LICENSE-APACHE`). Dependencies are limited to
permissive licenses (checked with `cargo deny`).
