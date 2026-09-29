#!/usr/bin/env bash
set -u -o pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

config="${CALIBERATE_CONFIG:-$HOME/.config/caliberate/control-plane.toml}"
mode="copy"

usage() {
  cat <<'EOF'
Usage:
  bash scripts/add-books.sh [--copy|--reference] PATH [PATH...]

Examples:
  bash scripts/add-books.sh ~/Downloads/book.epub
  bash scripts/add-books.sh ~/Downloads/a.epub ~/Downloads/b.pdf
  bash scripts/add-books.sh ~/Downloads/Books
  bash scripts/add-books.sh ~/Downloads/Books ~/MoreBooks

Directories are scanned recursively.
Supported book extensions: epub, mobi, azw, azw3, pdf, docx

Default mode is --copy, which stores managed copies in Caliberate's configured
library_dir. Use --reference only when you intentionally want Caliberate to
leave files where they already are.
EOF
}

paths=()
while (($#)); do
  case "$1" in
    --copy)
      mode="copy"
      ;;
    --reference)
      mode="reference"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      while (($#)); do
        paths+=("$1")
        shift
      done
      break
      ;;
    -*)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      paths+=("$1")
      ;;
  esac
  shift
done

if (("${#paths[@]}" == 0)); then
  usage >&2
  exit 2
fi

if [[ ! -f "$config" ]]; then
  echo "ERROR: Caliberate config not found: $config" >&2
  exit 1
fi

echo "Caliberate bulk ingest"
echo "Config: $config"
echo "Mode: $mode"
if [[ "$mode" == "copy" ]]; then
  echo "Managed library: /drive/books/managed"
else
  echo "Reference mode: source files remain in place"
fi
echo

echo "Building calibredb..."
cargo build -q -p caliberate-app --bin calibredb || exit 1
calibredb="$repo_root/target/debug/calibredb"

declare -a files=()
declare -A seen=()

is_supported_book() {
  local path="$1"
  local name ext
  name="${path##*/}"
  [[ "$name" == *.* ]] || return 1
  ext="${name##*.}"
  ext="${ext,,}"
  case "$ext" in
    epub|mobi|azw|azw3|pdf|docx) return 0 ;;
    *) return 1 ;;
  esac
}

queue_file() {
  local path="$1"
  local canonical
  is_supported_book "$path" || return 0
  canonical="$(realpath -m -- "$path" 2>/dev/null || printf '%s' "$path")"
  if [[ -z "${seen[$canonical]+x}" ]]; then
    seen["$canonical"]=1
    files+=("$path")
  fi
}

for input in "${paths[@]}"; do
  if [[ -f "$input" ]]; then
    if ! is_supported_book "$input"; then
      echo "Skipping unsupported file: $input"
      continue
    fi
    queue_file "$input"
  elif [[ -d "$input" ]]; then
    while IFS= read -r -d '' file; do
      queue_file "$file"
    done < <(find "$input" -type f -print0)
  else
    echo "WARNING: path not found: $input" >&2
  fi
done

if (("${#files[@]}" == 0)); then
  echo "No supported ebook files found."
  exit 0
fi

echo "Found ${#files[@]} supported ebook file(s)."
echo

added=0
skipped=0
failed=0

for file in "${files[@]}"; do
  echo "==> $file"
  if output="$("$calibredb" --config "$config" add --path "$file" --mode "$mode" 2>&1)"; then
    printf '%s\n' "$output"
    if grep -q "Added book " <<<"$output"; then
      ((added+=1))
    elif grep -q "Skipped ingest" <<<"$output"; then
      ((skipped+=1))
    fi
  else
    printf '%s\n' "$output" >&2
    ((failed+=1))
  fi
  echo
done

echo "Bulk ingest complete."
echo "Added:   $added"
echo "Skipped: $skipped"
echo "Failed:  $failed"

if ((failed > 0)); then
  exit 1
fi
