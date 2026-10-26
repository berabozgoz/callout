# CallOut

Oyuncular için düşük gecikmeli sesli sohbet. Rust çekirdek, Tauri 2 + Svelte arayüz,
kendi barındırabileceğin QUIC tabanlı SFU sunucusu.

Durum: **Faz 0 — iskelet.** Henüz ses iletmiyor.

## Gereksinimler

- Rust (stable) ve `cargo install tauri-cli --version "^2"`
- Node.js 22+
- Windows: [Tauri önkoşulları](https://v2.tauri.app/start/prerequisites/) (WebView2 Windows 10/11'de hazırdır)
- Linux (yalnızca kütüphaneler/sunucu): `libasound2-dev`

## Çalıştırma

```sh
cargo test                                      # kütüphane testleri
cargo run -p callout-audio --example loopback   # mikrofon → kulaklık yerel döngü
cargo run -p callout-server                     # sunucu iskeleti
npm --prefix ui install
cargo tauri dev --config apps/callout-app/tauri.conf.json   # masaüstü uygulaması
```

Mimari ve yol haritası: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
Geliştirme kuralları: [`CLAUDE.md`](CLAUDE.md).

## Lisans

MIT veya Apache-2.0, tercihinize göre.
