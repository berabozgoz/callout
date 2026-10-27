#!/usr/bin/env bash
# CallOut sunucusunu bir Linux VPS'e kurar (Ubuntu/Debian, root olarak çalıştırın):
#   curl -fsSL https://raw.githubusercontent.com/berabozgoz/callout/main/deploy/install.sh | sudo bash
# Yaptıkları: son sürümün Linux binary'sini indirir, /usr/local/bin'e koyar, "callout" sistem
# kullanıcısı ve /var/lib/callout veri klasörü açar, systemd servisi kurar (her açılışta başlar),
# ufw açıksa 4433/udp'ye izin verir ve davet linkini basar.
set -euo pipefail

REPO="berabozgoz/callout"
PORT="${CALLOUT_PORT:-4433}"
BIN=/usr/local/bin/callout-server
DATA=/var/lib/callout

echo "==> Son sürüm indiriliyor"
URL="https://github.com/$REPO/releases/latest/download/callout-server-linux-x64"
curl -fsSL "$URL" -o /tmp/callout-server
install -m 0755 /tmp/callout-server "$BIN"

id -u callout >/dev/null 2>&1 || useradd --system --home "$DATA" --shell /usr/sbin/nologin callout
mkdir -p "$DATA" && chown callout:callout "$DATA"

PUBLIC_IP="${CALLOUT_PUBLIC_IP:-$(curl -fsSL https://api.ipify.org || true)}"

cat > /etc/systemd/system/callout.service <<UNIT
[Unit]
Description=CallOut sesli sohbet sunucusu
After=network-online.target
Wants=network-online.target

[Service]
User=callout
Group=callout
Environment=CALLOUT_PORT=$PORT
Environment=CALLOUT_DATA=$DATA
Environment=CALLOUT_PUBLIC_IP=$PUBLIC_IP
Environment=RUST_LOG=info
ExecStart=$BIN
Restart=always
RestartSec=2
NoNewPrivileges=true
ProtectSystem=strict
ReadWritePaths=$DATA
ProtectHome=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
UNIT

systemctl daemon-reload
systemctl enable --now callout
systemctl restart callout

if command -v ufw >/dev/null 2>&1 && ufw status | grep -q "Status: active"; then
  ufw allow "$PORT"/udp >/dev/null && echo "==> ufw: $PORT/udp açıldı"
fi
# Oracle Cloud imajları: ufw yok, iptables REJECT kuralı var; UDP portunu onun önüne ekle.
if command -v iptables >/dev/null 2>&1 && ! iptables -C INPUT -p udp --dport "$PORT" -j ACCEPT 2>/dev/null; then
  iptables -I INPUT 1 -p udp --dport "$PORT" -j ACCEPT
  command -v netfilter-persistent >/dev/null 2>&1 && netfilter-persistent save >/dev/null 2>&1
  echo "==> iptables: $PORT/udp açıldı"
fi

sleep 1
echo
echo "==> Sunucu çalışıyor. Davet linki:"
journalctl -u callout -n 20 --no-pager | grep -o 'callout://[^ ]*' | grep "$PUBLIC_IP" | tail -1 || \
  journalctl -u callout -n 20 --no-pager | grep -o 'callout://[^ ]*' | tail -1
echo
echo "Günlük: journalctl -u callout -f   |   Güncelleme: bu betiği yeniden çalıştırın"
