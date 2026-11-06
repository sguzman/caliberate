#!/usr/bin/env bash
# Deploy the Librera OPDS search compatibility fix to the existing Linux service.
# Run from the Caliberate checkout after the change is merged into main.
set -Eeuo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

unit="caliberate.service"
installed="$HOME/.cargo/bin/calibre-server"
compiled="$repo_root/target/release/calibre-server"
mbx_cargo="$HOME/.local/share/mbx/bin/cargo"
base="http://127.0.0.1:8080"
backup=""
replacement=""
rollback_needed=0

on_exit() {
    local status="$?"
    trap - EXIT
    set +e
    if [[ -n "$replacement" ]]; then
        rm -f -- "$replacement"
    fi
    if ((status != 0 && rollback_needed == 1)); then
        echo "DEPLOYMENT FAILED: restoring previous Caliberate executable..." >&2
        local restore
        restore="$(mktemp "$HOME/.cargo/bin/.calibre-server.restore.XXXXXX")"
        if [[ -n "$restore" ]] &&
            install -m 755 "$backup" "$restore" &&
            mv -f -- "$restore" "$installed" &&
            doas -n systemctl restart "$unit" &&
            systemctl is-active --quiet "$unit" &&
            curl -fsS --max-time 15 "$base/health" >/dev/null; then
            echo "ROLLBACK VERIFIED: previous service is healthy." >&2
        else
            echo "URGENT: automatic rollback could not be verified; backup: $backup" >&2
        fi
    fi
    exit "$status"
}
trap on_exit EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

echo "Checking existing Caliberate service..."
[[ "$(git branch --show-current)" == "main" ]] || {
    echo "Refusing deployment: checkout is not on main." >&2
    exit 1
}
[[ -z "$(git status --porcelain)" ]] || {
    echo "Refusing deployment: checkout has uncommitted changes." >&2
    exit 1
}
[[ -f "$installed" ]] || {
    echo "Refusing deployment: installed calibre-server binary is missing." >&2
    exit 1
}
[[ -x "$mbx_cargo" ]] || {
    echo "Refusing deployment: the mbx Cargo shim is unavailable at $mbx_cargo." >&2
    exit 1
}
command -v doas >/dev/null || {
    echo "Refusing deployment: doas is unavailable." >&2
    exit 1
}
[[ "$(systemctl show -p FragmentPath --value "$unit")" == "/etc/systemd/system/caliberate.service" ]] || {
    echo "Refusing deployment: unexpected systemd unit file." >&2
    exit 1
}
systemctl show -p ExecStart --value "$unit" | grep -Fq "$installed" || {
    echo "Refusing deployment: service does not use the expected installed binary." >&2
    exit 1
}
systemctl is-active --quiet "$unit" || {
    echo "Refusing deployment: existing Caliberate service is not active." >&2
    exit 1
}
curl -fsS --max-time 15 "$base/health" >/dev/null || {
    echo "Refusing deployment: existing Caliberate health check failed." >&2
    exit 1
}

echo "Updating main with fast-forward only..."
git pull --ff-only origin main
grep -Fq 'const LIBRERA_SEARCH_TYPE: &str = "application/atom+xml";' crates/server/src/opds.rs || {
    echo "Refusing deployment: Librera compatibility fix not in source." >&2
    exit 1
}

echo "Running focused OPDS regression test via the mbx Cargo shim..."
"$mbx_cargo" test --locked -p caliberate-server opds_search_links_support_opensearch_and_librera
echo "Building release calibre-server via the mbx Cargo shim..."
"$mbx_cargo" build --locked --release -p caliberate-app --bin calibre-server
[[ -x "$compiled" ]] || {
    echo "Refusing deployment: release binary was not produced at $compiled" >&2
    exit 1
}

backup_dir="$HOME/.local/state/caliberate/backups"
mkdir -p "$backup_dir"
backup="$backup_dir/calibre-server.$(date +%Y%m%d-%H%M%S).bak"
cp -p -- "$installed" "$backup"
replacement="$(mktemp "$HOME/.cargo/bin/.calibre-server.new.XXXXXX")"
install -m 755 "$compiled" "$replacement"

echo "Installing tested binary and restarting existing service..."
rollback_needed=1
mv -f -- "$replacement" "$installed"
replacement=""
doas -n systemctl restart "$unit"

healthy=0
for attempt in {1..15}; do
    if systemctl is-active --quiet "$unit" &&
        curl -fsS --max-time 5 "$base/health" >/dev/null; then
        healthy=1
        break
    fi
    sleep 1
done
((healthy == 1)) || {
    echo "New Caliberate failed its health check." >&2
    exit 1
}

library="$(curl -fsS --max-time 20 "$base/library")"
[[ "$library" == *'Search books'* ]] || {
    echo "New Caliberate did not serve the expected browser search interface." >&2
    exit 1
}
catalog="$(curl -fsS --max-time 20 "$base/opds")"
[[ "$catalog" == *'href="/opds/search.xml" rel="search" type="application/opensearchdescription+xml"'* ]] || {
    echo "OpenSearch discovery link is missing." >&2
    exit 1
}
[[ "$catalog" == *'href="/opds/search?q={searchTerms}" rel="search" type="application/atom+xml"'* ]] || {
    echo "Librera-compatible Atom search link is missing." >&2
    exit 1
}
rollback_needed=0
echo "SUCCESS: Caliberate is active, healthy, and advertises both OPDS search links."
echo "Previous executable retained at: $backup"
