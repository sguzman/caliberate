#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

config="${CALIBERATE_CONFIG:-$HOME/.config/caliberate/control-plane.toml}"
server="$repo_root/target/debug/calibre-server"
server_log="${CALIBERATE_LAN_LOG:-$HOME/.local/state/caliberate/lan-library-server.log}"

echo "Starting Caliberate LAN library launcher..."
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
server_args=(
  --config "$config"
  --host 0.0.0.0
  --port 8080
)
"$server" "${server_args[@]}" >"$server_log" 2>&1 &
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

if command -v curl >/dev/null 2>&1; then
  echo "Checking browser library..."
  library_html="$(curl -fsS "http://127.0.0.1:8080/library")"
  if ! grep -q "Caliberate Library" <<<"$library_html"; then
    echo "ERROR: /library did not return the Caliberate browser UI."
    exit 1
  fi
  if ! grep -q "Download " <<<"$library_html"; then
    echo "ERROR: /library has no download links."
    exit 1
  fi
  echo "Browser library check: PASSED."
fi

echo
echo "Caliberate LAN library server is HEALTHY."
echo "Use this in Voice Dream as the web-site URL:"
echo "  http://$lan_ip:8080/library"
echo
echo "Server log: $server_log"
echo "Leave this terminal open. Press Ctrl+C to stop the server."

wait "$server_pid"
