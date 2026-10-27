# CallOut — geliştirici kuralları

Oyuncular için düşük gecikmeli sesli sohbet. Rust çekirdek, Tauri 2 + Svelte 5 arayüz,
kendi SFU sunucumuz (QUIC). MVP yalnızca Windows. Tam mimari: `docs/ARCHITECTURE.md`.

## Oturum düzeni

- **Başlarken** `docs/DURUM.md` dosyasını oku: nerede kaldık, sırada ne var.
- **Bitirirken** `docs/DURUM.md` içindeki "Son oturum" ve "Sırada" bölümlerini güncelle; Faz
  tamamlanınca aşağıdaki "Faz durumu" listesini de işaretle.
- Sunucu IP'si, davet linki, SSH anahtar yolu gibi özel bilgiler `CLAUDE.local.md`'dedir
  (git'e girmez). Bunları `docs/`, `README.md` ya da commit mesajlarına yazma.
- Türkçe yaz; kod yorumları ve commit mesajları Türkçe.

## Yapı

```
crates/callout-proto   protokol tipleri (istemci + sunucu paylaşır)
crates/callout-audio   cpal yakalama/oynatma, Opus, jitter, karıştırıcı
crates/callout-net     quinn QUIC: kontrol akışı + ses datagramları
crates/callout-core    Engine: Command/Event motoru (ses iş parçacığı + ağ görevleri); examples/talk.rs headless istemci
crates/callout-server  SFU + oda + kimlik, tek binary
apps/callout-app       Tauri kabuğu: engine_command/get_settings/save_settings, "engine" olay kanalı, PTT kısayolu, tepsi
ui/                    Svelte 5 (runes) + TypeScript + Vite
```

## Değişmez kurallar

1. **cpal geri çağrısında yalnızca halka tampona kopyalama.** Bellek ayırma, kilit,
   `println!`/`tracing`, kodlama, ağ yok. İhlal = çıtırtı.
2. **Ses verisi webview'a geçmez.** Tauri yalnızca `callout_core::Command` / `Event` taşır.
3. **Protokol değişikliği = `callout-proto`.** Tel biçimi `postcard`; enum varyantları
   sonuna eklenir, uyumsuzlukta `PROTOCOL_VERSION` artar.
4. **Sunucu Opus'u çözmez.** SFU paketi `AudioPacket::stamp_sender` ile damgalayıp iletir.
5. **Lisans MIT OR Apache-2.0.** GPL/AGPL crate eklenmez. Yeni crate eklerken
   `cargo deny`/lisansını kontrol et.
6. `#![forbid(unsafe_code)]` kütüphane crate'lerinde kalır. Gerekirse ayrı, küçük bir
   crate'te izole edilir.
7. Hata tipleri: kütüphanede `thiserror`, uygulama/sunucu `main`'de `anyhow`.
8. Kapsam dışı istekler (overlay, video, ekran paylaşımı) koda değil
   `docs/ARCHITECTURE.md` "Sonrası" listesine yazılır.

## Komutlar

```
cargo test                                  # kütüphane crate'leri (default-members)
cargo clippy --workspace --exclude callout-app --all-targets -- -D warnings
cargo run -p callout-audio --example loopback   # mikrofon → kulaklık, gecikme ölçümü
cargo run -p callout-server                 # sunucu; davet linkini basar
cargo run -p callout-core --example talk -- "<davet>" Ad --create   # oda aç (kodu basar)
cargo run -p callout-core --example talk -- "<davet>" Ad --room KOD # odaya gir
cd ui && npm install && cd ../apps/callout-app && cargo tauri dev   # uygulama
```

Linux'ta `libasound2-dev` gerekir; `callout-app` yalnızca Windows'ta derlenir/CI'da test edilir.

## Faz durumu

- [x] Faz 0 — iskelet
- [x] Faz 1 — ses MVP: loopback, Opus, QUIC, jitter/karıştırıcı, oda kodları, Tauri arayüzü (27 Ekim 2026: iki PC uygulama ile konuştu; sunucu Oracle Always Free üzerinde internette, v0.1.2 Releases'te). Kapı testi (4 kişi 1 saat) bekliyor.
- [ ] Faz 2 — ses kalitesi (nnnoiseless, AGC, FEC, uyarlanabilir jitter)
- [ ] Faz 3 — sosyal katman (hesap, arkadaş, varlık, metin)
- [ ] Faz 4 — yayın (imzalı NSIS, güncelleme, çökme raporu)

## Kararlar (26 Ekim 2026)

SFU, QUIC/quinn, Opus 48 kHz 20 ms, cpal/WASAPI, Tauri+Svelte, yalnızca Windows,
cihaz anahtarı + takma ad kimliği (hesap Faz 3), self-host destekli sunucu
(kendinden imzalı sertifika + parmak izi, `callout://` davet linki), MIT/Apache-2.0,
hedef oda 5–10 kişi (üst sınır 16).

## Tauri notu

`beforeDevCommand` / `beforeBuildCommand` Tauri tarafından `apps/` klasöründen (tauri klasörünün
bir üstü) çalıştırılır; bu yüzden `cd ../ui` doğrudur, `../../ui` yanlıştır. `frontendDist` ise
`tauri.conf.json`'a göre çözülür (`../../ui/dist`).
