#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

echo "Available shell candidates:"
for shell in "${SHELL:-}" /bin/sh /bin/zsh /opt/homebrew/bin/fish /usr/local/bin/fish /usr/bin/fish; do
  if [ -n "$shell" ] && [ -f "$shell" ]; then
    echo "  - $shell"
  fi
done | sort -u

cargo test -p well-shell --test shell_startup --locked -- --ignored --nocapture --test-threads=1
