#!/usr/bin/env bash
# P2-25: cassette-backed DAG generation gate acceptance report.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runs="${1:-1}"
out_file="${2:-$repo_root/docs/reviews/generation-eval.md}"

if command -v pwsh >/dev/null 2>&1; then
  exec pwsh -NoProfile -File "$repo_root/scripts/generation-eval.ps1" -Runs "$runs" -OutFile "$out_file"
fi

cd "$repo_root"
export GENERATION_EVAL_OUT="$out_file"
export GENERATION_EVAL_ITERATIONS="$runs"
cargo test -p generator generation_eval_write_report_when_env_set -- --exact --nocapture
echo "wrote $out_file"
