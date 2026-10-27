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

_(test sonrası doldurulacak)_
