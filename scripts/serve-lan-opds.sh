#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

config="${CALIBERATE_CONFIG:-$HOME/.config/caliberate/control-plane.toml}"
server="$repo_root/target/debug/calibre-server"

if [[ ! -x "$server" ]]; then
  echo "calibre-server binary not found; building it..."
  cargo build -p caliberate-app --bin calibre-server
fi

lan_ip="$(hostname -I 2>/dev/null | awk '{print $1}')"
if [[ -z "$lan_ip" ]]; then
  lan_ip="YOUR-PC-LAN-IP"
fi

echo
echo "Caliberate LAN OPDS server"
echo "Config: $config"
echo "Managed library: /drive/books/managed"
echo
echo "On your iPhone, try:"
echo "  http://$lan_ip:8080/opds"
echo
echo "The server will stay running in this terminal."
echo "Press Ctrl+C here when you want to stop it."
echo

exec "$server" \
  --config "$config" \
  --host 0.0.0.0 \
  --port 8080
