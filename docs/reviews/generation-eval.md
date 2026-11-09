# DAG generation gate evaluation (P2-25)

Mode: **cassette** · iterations per chain: **1** · target: **≥70%** first and final

| Metric | Pass | Total | Rate |
|---|---:|---:|---:|
| First gate accept | 3 | 3 | 100.0% |
| Final accept (after feedback) | 3 | 3 | 100.0% |

| Chain | Run | First pass | Final | LLM attempts |
|---|---:|:---:|:---:|---:|
| serial | 1 | yes | yes | 1 |
| async-poll | 1 | yes | yes | 1 |
| parallel | 1 | yes | yes | 1 |

**Verdict**: PASS (both rates ≥ 70%)


_Regenerate: `scripts/generation-eval.sh` (cassette). Live LLM: set `AITEST_LIVE_LLM=1` and provider env (not in default CI). Target reference: 70%._
