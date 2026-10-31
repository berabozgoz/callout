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

### 30–31 Ekim 2026 — bir saatlik ikili test (v0.1.5: Bera + at)

- **23:06–00:12 UTC (66 dk) kesintisiz**, ikisi de aynı bağlantıda kaldı; kapanış temiz (`bye`), hiç
  `ERROR` yok. Sunucu RTT: at ort. 39 ms (max 52), Bera ort. 45 ms (max 52). QUIC kaybı: at 47, Bera 0.
  İstemci `lost`: at 27 (13 kısa patlama, en büyüğü 5 paket), Bera 1.
- **Sürüm düzeltmesi sahada doğrulandı:** loglarda `version="0.1.5"`.
- **Jitter tamponu asimetrik:** at'ın tamponu (Bera'yı dinlerken) %89 oranında 40 ms. Bera'nınki (at'ı
  dinlerken) 40–240 ms arasında dalgalandı, ort. 136 ms; 23:15–23:57 arası çoğunlukla 180–240 ms. Yani
  at → Bera yönünde paketler düzensiz geliyor (at'ın yükleme hattı ya da ses cihazının paketleri
  topluca vermesi olası; at'ın mikrofon tabanı −86…−100 dB, alışılmadık). Bera yönünde uçtan uca
  gecikme tepe noktalarda ~300 ms'ye çıkabilir. Hangi gönderende ne kadar geç/eksik paket olduğu
  günlükte yok → gönderen başına jitter istatistiği eklenmeli (Faz 2 uyarlanabilir jitter girdisi).
- **Bir istem dışı kopma (v0.1.4, 22:57 UTC):** at odadan ayrıldıktan ~1 dk sonra, lobideyken istemci
  `bağlantı koptu: akış okuma: connection lost`, sunucu 11 sn sonra `TimedOut`. Sunucu RTT'si kopmadan
  önceki dakikada 36 → 49 ms tırmandı. Nedeni belirsiz; istemci günlüğü QUIC'in iç hata nedenini
  yazmıyor ("connection lost" ardındaki `ConnectionError`) → eklenmeli.
- **Geçti ölçütlerine göre:** 1 saat ✓, kopma ≤1 ✓ (lobide, odada değil), sunucu CPU ✓; **4 kişi ✗**
  (en çok 3 kişi, ~17 dk); **RTT + tampon < 150 ms: at tarafında ✓ (~80 ms), Bera tarafında ✗**
  (ort. ~180 ms).
- **Katılımcı cevapları:** (1) yüksek tampon aralığında birlikte Counter-Strike oynanıyordu, Bera gecikmeyi
  pek fark etmedi; (2) at Bluetooth kulaklık (AirPods) kullanıyor — mikrofon açıkken Windows "eller
  serbest" profiline geçer, ses paketleri topluca/düzensiz gelir: Bera'daki 180–240 ms tamponun ve
  at'taki "çıkış cihazı kayboldu" hatasının (22:18) en olası nedeni; (3) 22:57 kopması uygulama değil:
  at'ın bilgisayarı siyah ekran verdi, iki kez yeniden başlatıldı.

### 31 Ekim 2026 — bir saatlik ikili test, kablolu mikrofon (v0.1.5: Bera + "kuzen", farklı ağ)

- **13:58–15:10 UTC (72 dk) kesintisiz**; iki kapanış da temiz (`bye`), `ERROR` yok. Sunucu RTT: Bera ort.
  36 ms (max 47), kuzen ort. 38 ms (max 61). QUIC kaybı: kuzen 9, Bera 0. İstemci `lost`: Bera 2, kuzen 1.
- **Jitter tamponu bu kez simetrik ve düşük:** Bera (kuzeni dinlerken) ort. 53 ms, max 100, zamanın %53'ü
  40 ms; kuzen (Bera'yı dinlerken) ort. 44 ms, max 100, %82'si 40 ms. 10 dk'lık dilimlerde Bera'nın
  ortalaması 40–61 ms arasında kaldı, yükselme eğilimi yok.
- **AirPods karşılaştırması:** aynı Bera tarafında, AirPods'lu "at"ı dinlerken ort. 136 ms / max 240 ms,
  kablolu mikrofonlu "kuzen"i dinlerken ort. 53 ms / max 100 ms. Kişi ve ağ farklı olduğu için kesin
  değil, ama Bluetooth mikrofonun gönderen tarafta düzensizlik yarattığı teorisini güçlü biçimde
  destekliyor.
- **Geçti ölçütlerine göre:** 1 saat ✓, kopma 0 ✓, RTT + tampon < 150 ms ✓ her iki yönde (~89 ms ve
  ~80 ms). Eksik kalan tek ölçüt: 4 kişi.
- Küçük notlar: kuzen başta çıkış cihazını iki kez değiştirdi (44,1 kHz ↔ 48 kHz), ses hattı sorunsuz
  yeniden kuruldu. İstemci `lost` sayacı bir kez 1 → 0'a düştü: 60 sn sessiz kalan gönderen
  tablodan silinince onun kaybı toplamdan çıkıyor (istatistik eksikliği, ses sorunu değil).

