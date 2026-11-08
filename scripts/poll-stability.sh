#!/usr/bin/env bash
# Run examples/async-poll.yaml repeatedly (P0-54). Thin wrapper around poll-stability.ps1 on Windows,
# native loop on Unix.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runs="${1:-10}"
jitter_ms="${2:-40}"
seed="${3:-42}"
out_file="${4:-}"

if command -v pwsh >/dev/null 2>&1; then
  args=(-File "$repo_root/scripts/poll-stability.ps1" -Runs "$runs" -JitterMs "$jitter_ms" -Seed "$seed")
  if [ -n "$out_file" ]; then
    args+=(-OutFile "$out_file")
  fi
  exec pwsh -NoProfile "${args[@]}"
fi

cd "$repo_root"
cargo build -q -p mock-sut -p cli

listen="127.0.0.1:18888"
db_dir="$(mktemp -d)"
trap 'kill "$sut_pid" 2>/dev/null || true; rm -rf "$db_dir"' EXIT

mkdir -p "$db_dir/environments"
base_url=""

sut_out="$(mktemp)"
cargo run -q -p mock-sut -- --listen "$listen" --pay-delay-ms 80 --jitter-ms "$jitter_ms" --seed "$seed" >"$sut_out" 2>&1 &
sut_pid=$!
for _ in $(seq 1 120); do
  if base_url="$(grep -E '^https?://' "$sut_out" | head -1)"; then
    [ -n "$base_url" ] && break
  fi
  sleep 0.2
done
[ -n "$base_url" ] || { cat "$sut_out"; exit 1; }

printf 'base_url: %s\n' "$base_url" >"$db_dir/environments/local.yaml"
mytool="$repo_root/target/debug/mytool"
dag="$repo_root/examples/async-poll.yaml"

printf '| Run | Exit | PollAttempts | WaitDurationMs |\n'
printf '|---:|---:|---:|---:|\n'

passed=0
attempts=()
for i in $(seq 1 "$runs"); do
  report="$db_dir/run-$i.json"
  set +e
  AITEST_DB="$db_dir/aitest.db" "$mytool" run --dag "$dag" --env local --env-root "$db_dir" --no-llm \
    --report-json "$report" >/dev/null 2>&1
  code=$?
  set -e
  pa=""
  dm=""
  if [ -f "$report" ]; then
    pa="$(python3 - <<'PY' "$report"
import json, sys
r = json.load(open(sys.argv[1]))
p = r.get("deterministic", {}).get("nodes", {}).get("wait_paid", {})
poll = p.get("poll") or []
print(len(poll))
PY
)"
    dm="$(python3 - <<'PY' "$report"
import json, sys
r = json.load(open(sys.argv[1]))
t = r.get("volatile", {}).get("nodes", {}).get("wait_paid", {}).get("timing", {})
print(t.get("duration_ms", ""))
PY
)"
    attempts+=("$pa")
  fi
  [ "$code" -eq 0 ] && passed=$((passed + 1))
  printf '| %s | %s | %s | %s |\n' "$i" "$code" "$pa" "$dm"
done

echo "pass_rate: $passed/$runs"
