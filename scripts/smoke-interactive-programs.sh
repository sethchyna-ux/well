#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

missing=()
for program in vim less ssh tmux top; do
  if ! command -v "$program" >/dev/null 2>&1; then
    missing+=("$program")
  fi
done

if ((${#missing[@]} > 0)); then
  echo "Missing required interactive programs: ${missing[*]}" >&2
  exit 1
fi

cargo test -p well-shell --test interactive_programs --locked -- --ignored --nocapture --test-threads=1
