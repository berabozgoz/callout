#!/usr/bin/env bash
# Installs the CallOut server on a Linux VPS (Ubuntu/Debian, run as root):
#   curl -fsSL https://raw.githubusercontent.com/berabozgoz/callout/main/deploy/install.sh | sudo bash
# What it does: downloads the latest release's Linux binary, puts it in /usr/local/bin, creates
# the "callout" system user and the /var/lib/callout data directory, installs a systemd service
# (starts on every boot), allows 4433/udp if ufw is active, and prints the invite link.
set -euo pipefail

REPO="berabozgoz/callout"
PORT="${CALLOUT_PORT:-4433}"
BIN=/usr/local/bin/callout-server
DATA=/var/lib/callout

echo "==> Downloading the latest release"
URL="https://github.com/$REPO/releases/latest/download/callout-server-linux-x64"
curl -fsSL "$URL" -o /tmp/callout-server
install -m 0755 /tmp/callout-server "$BIN"

id -u callout >/dev/null 2>&1 || useradd --system --home "$DATA" --shell /usr/sbin/nologin callout
mkdir -p "$DATA" && chown callout:callout "$DATA"

PUBLIC_IP="${CALLOUT_PUBLIC_IP:-$(curl -fsSL https://api.ipify.org || true)}"

cat > /etc/systemd/system/callout.service <<UNIT
[Unit]
Description=CallOut voice chat server
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
  ufw allow "$PORT"/udp >/dev/null && echo "==> ufw: opened $PORT/udp"
fi
# Oracle Cloud images: no ufw, but an iptables REJECT rule; insert the UDP port ahead of it.
if command -v iptables >/dev/null 2>&1 && ! iptables -C INPUT -p udp --dport "$PORT" -j ACCEPT 2>/dev/null; then
  iptables -I INPUT 1 -p udp --dport "$PORT" -j ACCEPT
  command -v netfilter-persistent >/dev/null 2>&1 && netfilter-persistent save >/dev/null 2>&1
  echo "==> iptables: opened $PORT/udp"
fi

sleep 1
echo
echo "==> Server is running. Invite link:"
journalctl -u callout -n 20 --no-pager | grep -o 'callout://[^ ]*' | grep "$PUBLIC_IP" | tail -1 || \
  journalctl -u callout -n 20 --no-pager | grep -o 'callout://[^ ]*' | tail -1
echo
echo "Logs: journalctl -u callout -f   |   Update: run this script again"
