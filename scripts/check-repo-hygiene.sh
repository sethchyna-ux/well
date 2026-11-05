#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1; then
    echo "error: Cargo.lock must remain tracked for reproducible application builds" >&2
    exit 1
fi

tracked_ignored=()
while IFS= read -r -d '' path; do
    if [[ -e "$path" || -L "$path" ]]; then
        tracked_ignored+=("$path")
    fi
done < <(git ls-files -ci -z --exclude-standard)

if (( ${#tracked_ignored[@]} > 0 )); then
    echo "error: files matching .gitignore are still tracked:" >&2
    printf '%s\n' "${tracked_ignored[@]}" >&2
    exit 1
fi

echo "Repository hygiene check passed: Cargo.lock is tracked and no ignored files are tracked."
