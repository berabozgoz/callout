# CallOut

Oyuncular için düşük gecikmeli sesli sohbet. Rust çekirdek, Tauri 2 + Svelte arayüz,
kendi barındırabileceğin QUIC tabanlı SFU sunucusu.

Durum: **Faz 1 tamam.** Masaüstü uygulamasıyla bağlanıp oda açılıp konuşulabiliyor; sırada Faz 2 (ses kalitesi).

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

## Arkadaşlarla test (sürüm çıkarma)

```sh
cargo tauri build          # apps/callout-app içinde; target/release/bundle/nsis/CallOut_*_x64-setup.exe
cargo build --release -p callout-server   # target/release/callout-server.exe
```

Ya da bir etiket at, GitHub Actions kurulum dosyasını Releases sayfasına koysun:

```sh
git tag v0.1.0 && git push origin v0.1.0
```

Sunucuyu internete açmak: modemde **UDP 4433** portunu sunucu bilgisayarına yönlendir, dış IP'ni öğren
(tarayıcıda "what is my ip") ve sunucuyu `CALLOUT_PUBLIC_IP=<dış-ip> callout-server.exe` ile başlat;
basılan `callout://<dış-ip>:4433#...` linkini arkadaşlarına gönder. Windows Güvenlik Duvarı ilk seferde
izin ister. Kurulum dosyası imzasız olduğu için SmartScreen "bilinmeyen yayımcı" uyarısı verir
("Daha fazla bilgi → Yine de çalıştır"); kod imzalama Faz 4'te.

Mimari ve yol haritası: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
Geliştirme kuralları: [`CLAUDE.md`](CLAUDE.md).

## Lisans

MIT veya Apache-2.0, tercihinize göre.
