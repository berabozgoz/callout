# CallOut — güncel durum

> Her oturum başında bu dosyayı oku; oturum sonunda "Son oturum" ve "Sırada" bölümlerini güncelle.
> Sunucu adresi, davet linki gibi gizli bilgiler buraya değil, git'e girmeyen `CLAUDE.local.md`'ye yazılır.

## Nerede kaldık (28 Ekim 2026)

- **Faz 0 ve Faz 1 tamam.** İki Windows PC masaüstü uygulamasıyla aynı ağda konuştu; oda kodları,
  konuşma göstergesi, jitter 40–60 ms, LAN'da 0 kayıp.
- **Sunucu internette:** Oracle Always Free (Frankfurt, VM.Standard.E2.1.Micro, Ubuntu 24.04),
  `deploy/install.sh` ile systemd servisi olarak kuruldu. Bera uygulamadan bağlandı: RTT 36 ms.
  Bağlantı bilgileri `CLAUDE.local.md`'de.
- **Sürüm v0.1.2** GitHub Releases'te: NSIS kurulum dosyası (currentUser), taşınabilir
  `callout-app.exe`, `callout-server.exe`, `callout-server-linux-x64`.
- Ev ağı CGNAT olduğundan evden sunucu açmak mümkün değil; Tailscale istenmedi.

## Sırada

1. **Faz 1 kapı testi:** 4 kişi, 1 saat, Oracle sunucusu üzerinden. Gecikme, kopma, ses
   sorunlarını not et.
2. **Faz 2 — ses kalitesi** (sırayla):
   1. Gürültü bastırma: `nnnoiseless` (RNNoise, 48 kHz, 10 ms = 480 örnek) VAD'dan önce;
      ayarlarda aç/kapa.
   2. AGC: hedef −20 dBFS, yumuşak kazanç.
   3. Otomatik yeniden bağlanma (üstel geri çekilme, aynı odaya geri gir).
   4. UDP engelli ağ için yedek: ses paketlerini güvenilir QUIC akışından taşıyan mod.
   5. Kayıplı ağ simülasyonu testi (%5 kayıp, 100 ms jitter) — headless.
   6. UI cilası: VAD eşik payı ayarı, mikrofon test düğmesi, bağlantı kopunca uyarı.
3. Bilinen eksikler: PTT oyun içi (tam ekran) doğrulanmadı; fare tuşu PTT yok;
   `LICENSE-APACHE` tam metin eksik; CI "Kütüphaneler (Linux)" işi bir kez exit 1 verdi
   (yerelde tüm adımlar geçiyor, log bekleniyor).

## Çalışma düzeni

- Sunucu güncelleme: etiket at (`git tag v0.1.x && git push origin v0.1.x`), Actions bitince
  sunucuda `curl -fsSL https://raw.githubusercontent.com/berabozgoz/callout/main/deploy/install.sh | sudo bash`.
- Kod değişikliğinden sonra: `cargo fmt && cargo clippy --workspace --exclude callout-app --all-targets -- -D warnings && cargo test`.
- Protokol değişirse `callout-proto` + `PROTOCOL_VERSION`; istemci ve sunucu birlikte güncellenir.

## Son oturum

- 27–28 Ekim: `deploy/install.sh`'e Oracle (iptables) desteği eklendi; Oracle instance kuruldu,
  public IP ve UDP 4433 ingress kuralı eklendi; uygulama buluttaki sunucuya bağlandı.
