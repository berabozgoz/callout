# ADR 0001 — SFU topolojisi ve QUIC taşıma

Tarih: 2026-10-26 · Durum: kabul edildi

## Bağlam
Oyuncular için 5–10 kişilik düşük gecikmeli sesli sohbet. P2P'de N-1 yükleme ve NAT geçişi
sorun; WebRTC yığını tarayıcı olmayan istemci için gereksiz karmaşık.

## Karar
Kendi Rust SFU'muz; istemci-sunucu arası tek QUIC bağlantısı (quinn). Kontrol mesajları güvenilir
akışta, Opus paketleri datagramlarda. Sunucu Opus'u çözmez.

## Sonuçlar
+ Sunucu CPU'su düşük, self-host kolay, uçtan uca şifreleme sonradan eklenebilir.
− UDP'nin engellendiği ağlar için güvenilir akış üzerinden yedek mod gerekir (Faz 2).
− Tarayıcı istemcisi istenirse ayrı WebRTC köprüsü gerekir.
