#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-.}"
LIMIT="${2:-400}"

violations=0
while IFS= read -r file; do
  lines=$(wc -l < "$file")
  if [ "$lines" -gt "$LIMIT" ]; then
    printf '%6d %s\n' "$lines" "$file"
    violations=1
  fi
done < <(rg --files "$ROOT" -g '*.rs' | grep -v '/target/' | sort)

if [ "$violations" -ne 0 ]; then
  echo "Rust files above ${LIMIT} lines detected." >&2
  exit 1
fi

echo "All Rust files are within ${LIMIT} lines."
