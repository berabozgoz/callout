# CallOut

Oyuncular için düşük gecikmeli sesli sohbet. Rust çekirdek, Tauri 2 + Svelte arayüz,
kendi barındırabileceğin QUIC tabanlı SFU sunucusu.

Durum: **Faz 1, adım 6.** Masaüstü uygulamasıyla bağlanıp oda açılıp konuşulabiliyor (doğrulama sürüyor).

## Gereksinimler

- Rust (stable) ve `cargo install tauri-cli --version "^2"`
- Node.js 22+
- Windows: [Tauri önkoşulları](https://v2.tauri.app/start/prerequisites/) (WebView2 Windows 10/11'de hazırdır)
- Linux (yalnızca kütüphaneler/sunucu): `libasound2-dev`

## Çalıştırma

```sh
cargo test                                      # kütüphane testleri
cargo run -p callout-audio --example loopback   # mikrofon → kulaklık yerel döngü
cargo run -p callout-server                     # sunucu; callout:// davet linkini basar
cargo run -p callout-core --example talk -- "<davet>" Ad --create     # oda aç
cargo run -p callout-core --example talk -- "<davet>" Ad --room KOD   # odaya gir
cd ui && npm install && cd ..
cd apps/callout-app && cargo tauri dev                 # masaüstü uygulaması
```

Mimari ve yol haritası: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
Geliştirme kuralları: [`CLAUDE.md`](CLAUDE.md).

## Lisans

MIT veya Apache-2.0, tercihinize göre.
