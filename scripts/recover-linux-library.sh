#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

config="${CALIBERATE_CONFIG:-$HOME/.config/caliberate/control-plane.toml}"
report="${CALIBERATE_RECOVERY_REPORT:-$HOME/.local/state/caliberate/linux-recovery-report.txt}"

if [[ ! -f "$config" ]]; then
  echo "Caliberate config not found: $config" >&2
  exit 1
fi

mkdir -p "$(dirname "$report")"

echo "Building/running Caliberate recovery..."
cargo run -p caliberate-app --bin calibredb -- \
  --config "$config" \
  rebase-paths \
  --marker PHYSICALDRIVE0p1 \
  --replacement-root /drive \
  --apply \
  --report "$report"

echo
echo "Recovery report: $report"
echo "Paste that report back into ChatGPT if anything is not OK."
