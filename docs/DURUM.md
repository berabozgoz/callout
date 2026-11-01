# CallOut — güncel durum

> Her oturum başında bu dosyayı oku; oturum sonunda "Son oturum" ve "Sırada" bölümlerini güncelle.
> Sunucu adresi, davet linki gibi gizli bilgiler buraya değil, git'e girmeyen `CLAUDE.local.md`'ye yazılır.

## Nerede kaldık (1 Kasım 2026)

- **Faz 0 ve Faz 1 tamam** (kapı testi 1 Kasım'da kapandı; sırada Faz 2). İki Windows PC masaüstü uygulamasıyla aynı ağda konuştu; oda kodları,
  konuşma göstergesi, jitter 40–60 ms, LAN'da 0 kayıp.
- **Sunucu internette:** Oracle Always Free (Frankfurt, VM.Standard.E2.1.Micro, Ubuntu 24.04),
  `deploy/install.sh` ile systemd servisi olarak kuruldu. Bera uygulamadan bağlandı: RTT 36 ms.
  Bağlantı bilgileri `CLAUDE.local.md`'de.
- **Sürüm v0.1.6** GitHub Releases'te (sunucu da v0.1.6): NSIS kurulum dosyası (currentUser), taşınabilir
  `callout-app.exe`, `callout-server.exe`, `callout-server-linux-x64`.
- Ev ağı CGNAT olduğundan evden sunucu açmak mümkün değil; Tailscale istenmedi.

## Sırada

1. **Faz 1 kapandı (1 Kasım 2026).** Dört kişilik test (`KAPI_TESTI.md`): 4 kişi toplam ~52 dk
   (kesintisiz 35 dk), sistemde kopma/hata yok, gecikme ve CPU ölçütleri geçti; kesintisiz 60 dk'yı
   katılımcı tarafı kaçırdı. Faz 2 sırasına testten eklenenler: Opus FEC + PLC öne (gerçek ağlarda
   %1–8 kayıp), Bluetooth uyarısı, mikrofon izni reddinde (`0x80070005`) anlaşılır hata + yalnız dinleme
   girişi, panik kancası günlüğe.
2. **Faz 2 — ses kalitesi** (sırayla):
   1. Gürültü bastırma: `nnnoiseless` (RNNoise, 48 kHz, 10 ms = 480 örnek) VAD'dan önce;
      ayarlarda aç/kapa.
   2. AGC: hedef −20 dBFS, yumuşak kazanç.
   3. Otomatik yeniden bağlanma (üstel geri çekilme, aynı odaya geri gir).
   4. UDP engelli ağ için yedek: ses paketlerini güvenilir QUIC akışından taşıyan mod.
   5. Kayıplı ağ simülasyonu testi (%5 kayıp, 100 ms jitter) — headless.
   6. UI cilası: VAD eşik payı ayarı, mikrofon test düğmesi, bağlantı kopunca uyarı.
3. **Bas-konuş kısayolu tuşu yutuyordu** (28 Ekim, ikili testte bulundu): kısayol uygulama
   açıldığından beri sistem genelinde kayıtlıydı, düz "T" başka uygulamalarda yazılamıyordu.
   Düzeltildi: yalnızca bas-konuş açıkken **ve** odadayken kayıtlı (`sync_ptt`). Oda dışında
   (tepsi dahil) tuş serbest. Kalıcı çözüm Faz 2'de: tuşu tüketmeyen düşük seviye klavye/fare
   dinleyicisi (WH_KEYBOARD_LL; `unsafe` ayrı bir crate'te, kural 6) → oyun içinde düz tuş ve fare
   tuşu PTT. Odadayken düz tuş hâlâ yutulur; arayüzde uyarı notu var.
4. **Hayalet bağlantı:** düzeltildi (`Client::close_graceful`, tepsi "Çık"ta 700 ms bekleme) — gerçek
   uygulamada doğrulanmadı: sunucu günlüğünde kapanış `ApplicationClosed ... "bye"` olmalı, `TimedOut` değil.
   İlk ikili testte mamitest'in ilk oturumunda (odaya katıldı, ~7 sn sonra sunucuda `in_room=false`) neden
   açık: sunucuya katılma/ayrılma/mute günlükleri eklendi, aynı test mute/deafen ile tekrarlanacak.
5. **Ayarlarda tuş yakalama + F5 hatası** (v0.1.4 sonrası, 31 Ekim): bas-konuş alanı artık "bir tuşa
   bas" yakalayıcısı (`KeyboardEvent.code` → kısayol adı; yakalarken kısayol geçici bırakılır,
   `ptt_capture`). F5 webview'i yenileyip arayüzü sıfırlıyor, motor bağlı kalıyordu ("zaten bağlı"):
   app tarafında `Snapshot` + `engine_snapshot` komutu, arayüz açılışta kendini doldurur; motor bağlıyken
   `Connect` alırsa durumu yeniden yayınlar; yayın sürümünde F5/Ctrl+R/sağ tık engelli. Gerçek
   uygulamada denenmedi (commit edilmedi).
6. **v0.1.6 (31 Ekim):** ses cihazı kaybında otomatik kurtarma (geri çekilmeli; cihaz gidip gelirse
   30 sn kararlı olana kadar bekleme sıfırlanmaz), gönderen başına jitter günlüğü (30 sn'de bir
   `gönderen jitter sender=… target_ms late lost underflow`), kopma nedeninde QUIC'in asıl hatası,
   istemci `lost` sayacı artık geri düşmüyor. Cihaz çıkarma ile sahada henüz denenmedi.
7. Bilinen eksikler: PTT oyun içi (tam ekran) doğrulanmadı; fare tuşu PTT yok;
   `LICENSE-APACHE` tam metin eksik; CI "Kütüphaneler (Linux)" işi bir kez exit 1 verdi
   (yerelde tüm adımlar geçiyor, log bekleniyor).

## Çalışma düzeni

- **Sürüm numarası:** tek kaynak etiket. `release.yml` etiketten (`v0.1.5` → `0.1.5`) `Cargo.toml`
  `[workspace.package] version` değerini derleme öncesi yazar; kurulum dosyası adı
  (`CallOut_0.1.5_x64-setup.exe`) ve arayüz altbilgisi (`app.package_info()`) buradan gelir.
  `tauri.conf.json`'da `version` yok. Etiket `vX.Y.Z` biçiminde değilse akış hata verir. Repodaki
  `Cargo.toml` sürümünü ara sıra son etikete eşitle (yerel çalıştırmalarda görünen sürüm).
- Sunucu güncelleme: etiket at (`git tag v0.1.x && git push origin v0.1.x`), Actions bitince
  sunucuda `curl -fsSL https://raw.githubusercontent.com/berabozgoz/callout/main/deploy/install.sh | sudo bash`.
- Kod değişikliğinden sonra: `cargo fmt && cargo clippy --workspace --exclude callout-app --all-targets -- -D warnings && cargo test`.
- Protokol değişirse `callout-proto` + `PROTOCOL_VERSION`; istemci ve sunucu birlikte güncellenir.

## Son oturum

- 31 Ekim–1 Kasım: v0.1.5 ve v0.1.6 yayınlandı (tuş yakalama, F5 durumu, sürüm etiketten, ses cihazı
  kurtarma, gönderen başına jitter günlüğü, QUIC kopma nedeni); sunucu v0.1.6'ya güncellendi.
  Kablolu ikili 1 saatlik test geçti (31 Ekim); dört kişilik test yapıldı ve analiz edildi; Faz 1 kapatıldı (1 Kasım).
  Cihaz kurtarma sahada hâlâ denenmedi.

## Önceki oturum

- 27–28 Ekim: `deploy/install.sh`'e Oracle (iptables) desteği eklendi; Oracle instance kuruldu,
  public IP ve UDP 4433 ingress kuralı eklendi; uygulama buluttaki sunucuya bağlandı.
