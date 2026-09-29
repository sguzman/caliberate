#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

config="${CALIBERATE_CONFIG:-$HOME/.config/caliberate/control-plane.toml}"
report="${CALIBERATE_OPDS_REPORT:-$HOME/.local/state/caliberate/opds-smoke-report.txt}"
server_log="${CALIBERATE_OPDS_LOG:-$HOME/.local/state/caliberate/opds-smoke-server.log}"

mkdir -p "$(dirname "$report")"

echo "Building calibre-server..."
cargo build -p caliberate-app --bin calibre-server

server="$repo_root/target/debug/calibre-server"
echo "Starting local OPDS server..."
"$server" --config "$config" >"$server_log" 2>&1 &
server_pid=$!

cleanup() {
  if kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

health=""
for _ in $(seq 1 30); do
  if health="$("$server" --config "$config" health 2>/dev/null)"; then
    break
  fi
  if ! kill -0 "$server_pid" 2>/dev/null; then
    echo "Server exited before becoming healthy." >&2
    tail -n 50 "$server_log" >&2 || true
    exit 1
  fi
  sleep 1
done

if [[ -z "$health" ]]; then
  echo "Server did not become healthy." >&2
  tail -n 50 "$server_log" >&2 || true
  exit 1
fi

root="$("$server" --config "$config" opds-root)"
books="$("$server" --config "$config" opds-books)"

{
  echo "Caliberate OPDS smoke test"
  echo "status: OK"
  echo "config: $config"
  echo "local OPDS URL: http://127.0.0.1:8080/opds"
  echo
  echo "health:"
  printf '%s\n' "$health"
  echo
  echo "OPDS root bytes: ${#root}"
  echo "OPDS books bytes: ${#books}"
  echo
  echo "OPDS root preview:"
  printf '%s\n' "$root" | head -n 20
} >"$report"

cat "$report"
echo
echo "Report written: $report"
