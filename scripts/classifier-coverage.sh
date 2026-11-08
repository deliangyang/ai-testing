#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:-$repo_root/docs/reviews/classifier-coverage.md}"
if command -v pwsh >/dev/null 2>&1; then
  exec pwsh -NoProfile -File "$repo_root/scripts/classifier-coverage.ps1" -OutFile "$out"
fi
echo "classifier-coverage requires PowerShell (pwsh) or run on CI linux with pwsh" >&2
exit 1
