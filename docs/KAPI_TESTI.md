# Faz 1 kapı testi — plan

Amaç: 4 kişi, 1 saat, internet üzerinden (Oracle Frankfurt sunucusu). Faz 2 sırasını bu testin
verisi belirler. Sunucu adresi ve davet linki `CLAUDE.local.md`'dedir; katılımcılara özel kanaldan gönderilir.

## Önce (test günü, Bera)

- [ ] `git tag v0.1.3 && git push origin v0.1.3`; Actions bitince Releases'te NSIS kurulum dosyası var mı bak.
- [ ] Sunucuyu güncelle (`CLAUDE.local.md` → Güncelleme); `journalctl -u callout -f` açık dursun.
- [ ] Katılımcılara: kurulum dosyası + davet linki + bu belgedeki "Katılımcı" bölümü.
- [ ] İkinci terminalde `top` (CPU) ve `ss -u -a | grep 4433` ile yükü izle.

## Katılımcı

1. Kurulum dosyasını çalıştır, uygulamayı aç, davet linkini yapıştır, takma ad yaz.
2. Kulaklık kullan (hoparlör yankı yapar; yankı bastırma henüz yok).
3. Testten sonra şu dosyayı Bera'ya gönder:
   `%APPDATA%\io.github.berabozgoz.callout\logs\callout.log`
4. Aşağıdaki formu doldur (ağ tipi, gözlemler).

## Akış (60 dk)

| Dk | Ne | Bakılacak |
|----|----|-----------|
| 0–10 | Herkes odaya girer (1 kişi oda açar, kod paylaşılır), serbest sohbet | Giriş sorunu, ilk ses |
| 10–25 | Serbest sohbet, biri oyun açar (tam ekran) | Çıtırtı, CPU etkisi |
| 25–35 | Bas-konuş (PTT) modu; en az bir kişi oyun içinde PTT dener | Oyun içi PTT çalışıyor mu |
| 35–45 | Herkes aynı anda konuşur (4 ses karışımı) | Karışma, robotlaşma |
| 45–55 | Bilerek: bir kişi Wi-Fi ↔ kablo geçer, bir kişi bağlantıyı kesip tekrar girer | Kopma uyarısı, yeniden giriş |
| 55–60 | Odadan çıkış, herkes formu doldurur | — |

## Geçti ölçütü

- 1 saatte istem dışı kopma ≤ 1 (kendiliğinden toparlanan sayılmaz; şu an otomatik yeniden bağlanma yok).
- Ortalama RTT + tampon < 150 ms (uçtan uca algı: "gecikme hissetmedim").
- Çıtırtı/robotlaşma yalnızca nadir; "konuşuyorum ama duyulmuyorum" bildirimi yok.
- Sunucu CPU'su 4 kişide %50'nin altında.

## Katılımcı formu

- Takma ad: ______  Ağ: ev kablo / ev Wi-Fi / mobil veri / iş-okul  Operatör: ______
- Gecikme hissi (1 rahat – 5 çok kötü): ___
- Çıtırtı/kesilme/robot ses: yok / ara sıra / sık  Ne zaman: ______
- Kopma oldu mu? Saat ve ne yaptığın: ______
- Mikrofon seviyesi / VAD: kendini duyuruyor muydun? Başkaları seni?
- PTT oyun içinde çalıştı mı (oyun adı): ______
- Notlar: ______

## Sonrası: günlükleri okuma

- Sunucu: `journalctl -u callout --since "1 hour ago" | grep "bağlantı istatistiği"` →
  kullanıcı başına `rtt_ms`, `lost_packets` (QUIC), `dgram_rx/tx`. `bağlantı kapandı` satırında
  `close_reason` ve `secs` var: istem dışı kopma = beklenmedik `close_reason` / kısa `secs`.
- İstemci `callout.log`: 10 sn'de bir `istatistik` satırı (`rtt_ms`, `jitter_ms`, `lost` = jitter
  tamponunun toplam kaybı, `sent`/`received` = son 2 sn'deki paketler), ayrıca `bağlantı kapanıyor` nedeni.
- Sonuçları bu belgenin altına tarih + özet olarak ekle; Faz 2 sırasını (DURUM.md "Sırada") veriye göre güncelle.

## Sonuçlar

### 28 Ekim 2026 — ikili ön test (v0.1.3, Bera + mamitest, ikisi ethernet masaüstü)

- Ses iki yönde aktı; QUIC kaybı 0 (kesinti dışında), RTT Bera 37–42 ms, mamitest 45–51 ms,
  jitter tamponu 40–80 ms, sunucu CPU'su boşta.
- **Kablo çekme (~5–10 sn):** bağlantı kopmadı (aynı `user_id`, `max_idle_timeout` 10 sn'den kısa
  kaldığı için); sunucu 73 paket kayıp saydı, `cwnd` 12000 → 2904. Kullanıcı kesinti sırasında ve
  sonrasında ses/çıtırtı/sıçrama fark etmedi. >10 sn kesinti `TimedOut` olur, otomatik yeniden
  bağlanma yok.
- **Bulunan hatalar:** (1) bas-konuş kısayolu odadan sonra da tuşu yutuyordu (düzeltildi, `87a4f61`);
  (2) uygulama kapanırken kapatma çerçevesi bazen gitmiyor, bağlantı ~15 sn `TimedOut` ile siliniyor.
- **mamitest günlüğü (alındı):** ikinci oturumda RTT 45–50 ms, `lost=0`, jitter 40–80 ms, alım pencereleri
  69–100/100; kablo kesintisi onun günlüğünde de aynı anda görünüyor ve toparlanmış.
- **Açık gözlem (ilk oturum 18:39:59–18:40:31):** sunucu mamitest'i odaya aldı ("odaya katıldı") ama ~7 sn
  sonra istatistikte `in_room=false`; Bera onu duymadı, mamitest `sent=100` gönderdi. Kullanıcı o sırada
  uygulamayı kapatıp açtığını ve giriş ekranı gördüğünü söylüyor; mute/deafen düğmelerini kullanmıştı.
  Kodda mute/deafen'in odadan çıkaracağı bir yol bulunamadı. Sunucu çıkışı/bayrakları günlüğe yazmıyordu;
  v0.1.4'te "odadan ayrıldı", "mikrofon/kulaklık durumu" satırları eklendi — tekrar testte izlenecek.
- **Oda dışında ses gönderiliyordu** (sunucu atıyordu): düzeltildi, yalnızca odadayken gönderilir.
- Henüz yok: uzun süreli (1 saat) ve 4 kişilik yük, farklı ağlardan katılım.

### 30 Ekim 2026 — üç kişilik test (v0.1.4: Bera, "at" (kuzen, farklı ağ), mamitest)

- Üçü de ~20–50 dk odada; 3 kişi aynı anda konuşma çalıştı (alınan pencereler 180'e kadar).
  RTT: Bera ort. 45 ms (sunucu max 55), at 36–38 ms, mamitest 36–37 ms. Jitter tamponu ort. 44–63 ms,
  3 kişide en çok 100 ms. İstemci `lost`: at 21, Bera/mamitest 0. Sunucu QUIC kaybı: at 106 (~%0,1,
  kuzenin ağı), diğerleri 0.
- 22:17–22:25 UTC'deki 8 dk sessizlik (sent=0 received=0) ve 22:13'teki 30 sn sessizlik kesinti değil:
  sunucu günlüğüne göre ikisi de mute/deafen yapmıştı / kimse konuşmadı; RTT sabit.
- Mute/deafen onlarca kez açılıp kapandı, hiçbir zaman `in_room` düşmedi → ilk testteki `in_room=false`
  mute/deafen'den değil; hâlâ açıklanamadı.
- **Hata: çıkış cihazı kaybında ses kurtarılmıyor.** `çıkış hatası: The requested device is no longer
  available` — Bera 22:44:14, at 22:18:30. Motor yalnızca günlüğe yazıyor; çıkış akışı ölü kalıyor
  (Bera ~2,5 dk, at ~8 dk duyamadı), ses hattı elle cihaz seçilince yeniden kuruldu (Bera 22:46:50,
  at 22:26:33).
- mamitest'in bağlantısı odadan ayrıldıktan 18 sn sonra `TimedOut` ile kapandı (log kopyası uygulama
  açıkken alınmış, kapanış satırı yok); pencere X'i tepsiye gizler, yalnızca tepsi "Çık" kapatma
  çerçevesi gönderir.

