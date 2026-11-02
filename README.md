# CallOut

Low-latency voice chat for gamers. Rust core, Tauri 2 + Svelte UI,
and a QUIC-based SFU server you can host yourself.

Status: **Phase 1 done.** You can connect with the desktop app, open a room and talk; next up is Phase 2 (audio quality).

## Requirements

- Rust (stable) and `cargo install tauri-cli --version "^2"`
- Node.js 22+
- Windows: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) (WebView2 ships with Windows 10/11)
- Linux (libraries/server only): `libasound2-dev`

## Running

```sh
cargo test                                      # library tests
cargo run -p callout-audio --example loopback   # mic → headphones local loop
cargo run -p callout-server                     # server; prints the callout:// invite link
cargo run -p callout-core --example talk -- "<invite>" Name --create     # open a room
cargo run -p callout-core --example talk -- "<invite>" Name --room CODE  # join a room
cd ui && npm install && cd ..
cd apps/callout-app && cargo tauri dev                 # desktop app
```

## Testing with friends (cutting a release)

```sh
cargo tauri build          # inside apps/callout-app; target/release/bundle/nsis/CallOut_*_x64-setup.exe
cargo build --release -p callout-server   # target/release/callout-server.exe
```

Or push a tag and let GitHub Actions put the installer on the Releases page:

```sh
git tag v0.1.0 && git push origin v0.1.0
```

The easiest way to put the server on the internet is a VPS (Ubuntu 24.04, the smallest plan is enough); as root:

```sh
curl -fsSL https://raw.githubusercontent.com/berabozgoz/callout/main/deploy/install.sh | sudo bash
```

The script downloads the latest release, installs a systemd service, opens 4433/udp and prints the `callout://...`
invite link (logs via `journalctl -u callout -f`). Hosting from a home connection needs **UDP 4433** port forwarding
on the router and `CALLOUT_PUBLIC_IP=<external-ip>`; if your ISP uses CGNAT (the router's WAN IP differs from your
external IP) this will not work. The installer is unsigned, so SmartScreen shows an "unknown publisher" warning
("More info → Run anyway"); code signing comes in Phase 4.

Architecture and roadmap: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
Development rules: [`CLAUDE.md`](CLAUDE.md).

## License

MIT or Apache-2.0, at your option.
