# CallOut — Mimari ve yol haritası (özet)

Yaşayan tam belge (diyagramlar, gerekçeler, yorumlar):
https://claude.ai/artifact/Fm7ZdEpMeUJSY3tpw36Faq — son karar tarihi 26 Ekim 2026.
Bu dosya depoda taşınan özettir; protokol ayrıntısında **kod (`crates/callout-proto`) gerçeğin kaynağıdır.**

## Ürün

Oyuncular bir oda koduyla buluşur ve 150 ms altı uçtan uca gecikmeyle konuşur.
MVP yalnızca Windows. Hedef oda 5–10 kişi (üst sınır 16).

## Temel kararlar

| Karar | Seçim | Gerekçe |
| --- | --- | --- |
| Topoloji | Sunucu tabanlı SFU | İstemci 1 akış yükler, N-1 indirir; NAT sorunu sunucuda çözülür |
| Taşıma | QUIC (quinn + rustls) | Tek bağlantı: güvenilir kontrol akışı + güvenilmez ses datagramları; TLS 1.3 gömülü |
| Kodek | Opus 48 kHz, 20 ms, 24–32 kbps, FEC + DTX | Konuşma için şeffaf, kayba dayanıklı |
| Ses G/Ç | cpal (WASAPI) | Saf Rust; gerekirse `windows` crate ile exclusive mod |
| Arayüz | Tauri 2 + Svelte 5 | Ses ve ağ Rust'ta kalır; arayüz hızlı iterasyon alır |
| Kimlik | Ed25519 cihaz anahtarı + takma ad | Parola yok; e-posta hesabı Faz 3 |
| Barındırma | Self-host destekli | Aynı binary; kendinden imzalı sertifika + parmak izi; `callout://IP:port#parmak-izi` daveti |
| Lisans | MIT OR Apache-2.0 | GPL crate yok |

## Sistem

```
İstemci A (Tauri)            callout-server (VPS / ev PC)         İstemci B…N
┌──────────────────┐         ┌────────────────────────┐          ┌──────────────┐
│ Svelte arayüz    │         │ SFU: Opus'u çözmeden   │          │ Svelte       │
│ callout-core     │ ──ses──▶│ odadakilere kopyalar   │ ──ses──▶ │ callout-core │
│ callout-audio    │ ◀─ses── │ rooms · auth · (chat)  │ ◀─ses──  │ callout-audio│
│ callout-net/QUIC │ ◀kontrol▶ SQLite                │ ◀kontrol▶│ callout-net  │
└──────────────────┘         └────────────────────────┘          └──────────────┘
   ses = QUIC datagram (güvenilmez, 20 ms)   kontrol = QUIC akışı (güvenilir)
```

## İstemci

- `callout-proto` tipler · `callout-audio` cpal + Opus + jitter + karıştırıcı ·
  `callout-net` QUIC · `callout-core` durum makinesi · `callout-app` Tauri kabuğu · `ui/` Svelte.
- Gönderim: mikrofon → cpal geri çağrısı (yalnızca halka tampona kopya) → ses iş parçacığı:
  48 kHz'e yeniden örnekleme, 20 ms çerçeve, VAD, [Faz 2] gürültü bastırma, Opus → datagram.
- Alım: datagram → gönderene göre jitter tamponu (60 ms başlangıç, uyarlanabilir) → Opus çözme + PLC →
  kullanıcı başına kazanç → karıştırıcı (f32 toplama + yumuşak sınırlayıcı) → cpal oynatma.
- Tauri sınırı: arayüz `Command` gönderir, `Event` alır; ses verisi webview'a geçmez.

## Sunucu

Tek tokio süreci, tek UDP portu. Modüller: `sfu` (oda başına görev, `stamp_sender` ile damgalayıp iletir;
susturulmuş üyeden gelen paket iletilmez), `rooms`, `auth` (challenge → Ed25519 imza), `store` (sqlx/SQLite),
`admin` (sağlık + metrik). Faz 3: `presence`, `chat`. Odalar sunucuya sabittir; sunucular arası yönlendirme yok.

## Protokol

Kontrol: `[u32 LE uzunluk][postcard(ClientMsg|ServerMsg)]`, `PROTOCOL_VERSION` uyumsuzsa ret.
El sıkışma: `Hello` → `Welcome{challenge}` → `Auth{signature}` → `Authed`.
Ses paketi: `sender_id(4) seq(2) timestamp(4) flags(1) opus(≤400)`; ~52 kbps/konuşmacı, DTX ile sessizlikte 0.
Şifreleme: MVP'de QUIC/TLS taşıma şifrelemesi; uçtan uca için `flags::E2E` biti ayrılmıştır.

## Yol haritası

| Faz | Kapsam | Geçiş kapısı |
| --- | --- | --- |
| 0 İskelet (1 hafta) | workspace, proto, CI, Tauri boş kabuk, cihaz listesi | CI yeşil; uygulama cihazları listeliyor |
| 1 Ses MVP (4–6 hafta) | loopback → Opus → QUIC iki istemci → jitter/karıştırıcı → oda + self-host adresi → arayüz | 4 kişi 1 saat; gecikme ≤ 150 ms |
| 2 Ses kalitesi (3 hafta) | nnnoiseless, AGC, FEC, uyarlanabilir jitter, kişi sesi, yeniden bağlanma | %5 kayıp + 100 ms jitter'da anlaşılır; klavye duyulmaz |
| 3 Sosyal katman (4 hafta) | hesap, arkadaşlar, varlık, metin, kalıcı odalar | arkadaş ekle → çevrimiçi gör → odaya çağır |
| 4 Yayın (3 hafta) | imzalı NSIS, otomatik güncelleme, çökme raporu, kapalı beta | 20 kullanıcı 2 hafta; çökme < %1 |

Sonrası: Linux/macOS, oyun içi overlay, uçtan uca şifreleme, tarayıcı köprüsü, bölgesel sunucular,
tek tıkla self-host kurulum aracı.

## Riskler

cpal/WASAPI çıtırtısı (loopback ile erken ölç) · UDP engelli ağlar (güvenilir akış üzerinden yedek mod) ·
tek bölge gecikmesi (self-host + ileride çoklu sunucu) · SmartScreen (Faz 4 imza) · kapsam şişmesi (kapı kriterleri).

## Karar kayıtları (ADR)

Yeni bir mimari karar `docs/adr/NNNN-baslik.md` olarak eklenir: bağlam, karar, sonuçlar.
