#!/usr/bin/env bash
# Single quality gate for the repository. Must pass before every commit.
# Usage: scripts/check.sh [fmt] [clippy] [test] [guards]   (default: all)
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# Keep in sync with scripts/check.ps1 and AGENTS.md section 5.
deterministic_crates=(common model gate store classifier engine cli)
forbidden_crates=(llm-gateway generator autonomy)
sleep_allowed_files=(crates/common/src/clock.rs)
sleep_allowed_prefixes=(crates/mock-sut/)
sleep_pattern='\b(thread|time)::sleep\b|\btime::\{[^}]*\bsleep\b|\bsleep_until\b'

steps=("$@")
[ ${#steps[@]} -eq 0 ] && steps=(fmt clippy test guards)
failed=()

want() { local s; for s in "${steps[@]}"; do [ "$s" = "$1" ] && return 0; done; return 1; }

# On GitHub Actions a failing step's output tail becomes an error annotation, which is
# readable without downloading the (authenticated) job log.
annotate() {
  local name="$1" log="$2" tail
  [ "${GITHUB_ACTIONS:-}" = "true" ] || return 0
  tail="$(tail -n 60 "$log")"
  tail="${tail//'%'/'%25'}"
  tail="${tail//$'\r'/'%0D'}"
  tail="${tail//$'\n'/'%0A'}"
  echo "::error title=check failed: ${name}::${tail}"
}

run_step() {
  local name="$1" log status; shift
  echo "==> $name"
  log="$(mktemp)"
  "$@" 2>&1 | tee "$log"
  status=${PIPESTATUS[0]}
  if [ "$status" -ne 0 ]; then
    failed+=("$name")
    echo "    FAILED: $name"
    annotate "$name" "$log"
  fi
  rm -f "$log"
}

dependency_guard() {
  local ok=0 members crate forbidden tree
  members="$(cargo metadata --no-deps --format-version 1 | grep -o '"name":"[^"]*","version"' | cut -d'"' -f4)"
  for crate in "${deterministic_crates[@]}"; do
    grep -qx "$crate" <<<"$members" || continue
    tree="$(cargo tree -p "$crate" -e normal,build --prefix none --format '{p}')" || return 1
    for forbidden in "${forbidden_crates[@]}"; do
      if grep -q "^${forbidden} v" <<<"$tree"; then
        echo "    '$crate' depends on forbidden crate '$forbidden'"
        ok=1
      fi
    done
  done
  return $ok
}

sleep_guard() {
  local ok=0 file allowed prefix skip
  while IFS= read -r -d '' file; do
    file="${file#./}"
    skip=0
    for allowed in "${sleep_allowed_files[@]}"; do [ "$file" = "$allowed" ] && skip=1; done
    for prefix in "${sleep_allowed_prefixes[@]}"; do [[ "$file" == "$prefix"* ]] && skip=1; done
    [ $skip -eq 1 ] && continue
    if grep -nE "$sleep_pattern" "$file" | sed "s|^|    bare sleep at $file:|"; then
      ok=1
    fi
  done < <(find ./crates -name '*.rs' -not -path '*/target/*' -print0 2>/dev/null)
  return $ok
}

web_npm() {
  [ -f web/package.json ] || return 0
  (cd web && npm run "$1" --silent)
}

want fmt && run_step "cargo fmt" cargo fmt --all -- --check
want fmt && run_step "web: prettier" web_npm format:check
want clippy && run_step "cargo clippy" cargo clippy --workspace --all-targets -- -D warnings
want clippy && run_step "web: eslint" web_npm lint
want test && run_step "cargo test" cargo test --workspace
want test && run_step "web: vitest" web_npm test
want test && run_step "web: build" web_npm build
if want guards; then
  run_step "guard: deterministic crates must not depend on LLM crates" dependency_guard
  run_step "guard: no bare sleep outside Sleeper" sleep_guard
fi

if [ ${#failed[@]} -gt 0 ]; then
  printf '\ncheck failed: %s\n' "$(IFS='; '; echo "${failed[*]}")"
  exit 1
fi
printf '\nall checks passed\n'
