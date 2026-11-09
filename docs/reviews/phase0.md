# Phase 0 acceptance (`develop.md` §8.3)

M0 联合评审（§7.3）见 [`m0.md`](./m0.md)（Q1 轮询、Q2 证据草稿、Q3 闸门/分类器）。

**结论**：通过。下文每条对应阶段 0 摘录的两项验收，并给出可复现证据。

## 1. 同一 DAG 两次 `--no-llm` 运行，确定性报告逐字节一致

| 证据 | 命令 / 测试 |
|---|---|
| `examples/` 三条样例 + mock-sut | `cargo test -p cli --test golden` |
| wiremock 形态回归 | `cargo test -p cli --test run report_json_partitions_clocks_and_is_stable_across_runs` |
| CLI 无 LLM 依赖 | `cargo test -p cli --test golden cli_dependency_tree_excludes_llm_crates`；`scripts/check.ps1` guard |

## 2. 轮询超时轨迹完整可回放

| 证据 | 命令 / 测试 |
|---|---|
| 超时 poll 全轨迹 | `cargo test -p cli --test run run_show_replays_a_timed_out_poll_trace` |
| 引擎 poll 轨迹 | `cargo test -p engine poll_`（`crates/engine/tests/poll.rs`） |
| 持久化字段 | `cargo test -p store node_executions` |

## 3. 闸门与三条 spike 链路（阶段 0 能力面）

| 证据 | 命令 / 测试 |
|---|---|
| 坏 DAG 语料 100% 拦截 | `cargo test -p gate`；`cargo test -p cli --test dag_store every_bad_dag` |
| 官方样例可校验 | `cargo test -p cli --test dag_validate spike_examples_pass_gate` |
| 串行 / 并行 / 异步可运行 | `cargo test -p cli --test run`（三条 wiremock 用例）；`cargo test -p cli --test golden` |

## 4. Spike 脚本（M0 线 A / 分类器）

| 证据 | 命令 |
|---|---|
| 轮询 10 次统计 | `powershell -File scripts/poll-stability.ps1 -Runs 10 -OutFile docs/reviews/poll-stability.md` |
| 故障注入分类 | `powershell -File scripts/classifier-coverage.ps1` → `docs/reviews/classifier-coverage.md` |

## 遗留

- **M0 评审**：详见 [`m0.md`](./m0.md)。**Q2**：UI 证据见 **P1-29** E2E（`failure-evidence.spec.ts`）；Q2 草稿已写入 `m0.md`，**Q8** 待人类签收是否可作为 M0 正式结论。**Q3**：线 C 在阶段 0 以坏 DAG 100% 拦截 + 分类器 unknown（[`classifier-coverage.md`](./classifier-coverage.md)）为据；模型闸门 **≥70% 接受率** 在阶段 2 **P2-25** 验收，见 [`generation-eval.md`](./generation-eval.md) 与 [`phase2.md`](./phase2.md)。
- 阶段 1 任务在 **P0-57** 标 DONE 后按 `AGENTS.md` 阶段门启动。
