#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

config="${CALIBERATE_CONFIG:-$HOME/.config/caliberate/control-plane.toml}"
server="$repo_root/target/debug/calibre-server"
server_log="${CALIBERATE_LAN_LOG:-$HOME/.local/state/caliberate/lan-opds-server.log}"

echo "Starting Caliberate LAN OPDS launcher..."
echo "Config: $config"

echo "Building calibre-server..."
cargo build -p caliberate-app --bin calibre-server

mkdir -p "$(dirname "$server_log")"

lan_ip="$(
  ip -4 -o addr show up scope global 2>/dev/null |
    awk '{split($4,a,"/"); print a[1]; exit}'
)"
if [[ -z "$lan_ip" ]]; then
  lan_ip="YOUR-PC-LAN-IP"
fi

echo "Starting server on 0.0.0.0:8080..."
"$server" \\\n  --config "$config" \\\n  --host 0.0.0.0 \\\n  --port 8080 \\\n  >"$server_log" 2>&1 &
server_pid=$!

cleanup() {
  if kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

healthy=0
for _ in $(seq 1 10); do
  if "$server" --config "$config" --host 127.0.0.1 --port 8080 health >/dev/null 2>&1; then
    healthy=1
    break
  fi
  if ! kill -0 "$server_pid" 2>/dev/null; then
    echo "ERROR: calibre-server exited during startup."
    echo "Server log: $server_log"
    tail -n 30 "$server_log" || true
    exit 1
  fi
  sleep 1
done

if [[ "$healthy" -ne 1 ]]; then
  echo "ERROR: calibre-server did not become healthy."
  echo "Server log: $server_log"
  tail -n 30 "$server_log" || true
  exit 1
fi

echo "Checking OPDS navigation..."
root_feed="$("$server" --config "$config" --host 127.0.0.1 --port 8080 opds-root 2>/dev/null)"
if ! grep -q "All Books" <<<"$root_feed" || ! grep -q "Authors" <<<"$root_feed"; then
  echo "ERROR: OPDS root is reachable but navigation entries are missing."
  exit 1
fi

echo "Checking OPDS acquisition links..."
books_feed="$("$server" --config "$config" --host 127.0.0.1 --port 8080 opds-books 2>/dev/null)"
if ! grep -q "opds-spec.org/acquisition" <<<"$books_feed"; then
  echo "ERROR: OPDS books feed is reachable but acquisition links are missing."
  exit 1
fi

echo
echo "Caliberate LAN OPDS server is HEALTHY."
echo "OPDS navigation and acquisition checks: PASSED."
echo "On your iPhone, open:"
echo "  http://$lan_ip:8080/opds"
echo
echo "Server log: $server_log"
echo "Leave this terminal open. Press Ctrl+C to stop the server."

wait "$server_pid"
