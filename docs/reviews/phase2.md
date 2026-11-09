# Phase 2 acceptance (`develop.md` §8.3)

**阶段验收摘录（[`TASKS.md`](../../TASKS.md) 阶段 2，表头）**：含无效变量引用的 AI 候选入库前被拦截并定位；通过候选激活后行为与手工一致。

设计对照：[`develop.md`](../../develop.md) **§8.3** 阶段 2 行；阶段 2 里程碑表（§7.1 对应「AI 生成与校验闸门」）；M0 **Q3** 闸门接受率见 [`generation-eval.md`](./generation-eval.md)（**P2-25**，cassette 模式）。

任务 **P2-27** 依赖 **P2-13**、**P2-23～P2-26**（不含 **P2-24** AI 副驾，仍为 `TODO`，记入遗留）。

**结论**：**通过**。§8.3 两条核心标准有自动化证据；阶段 2 质量门 `./scripts/check.sh` exit 0（2026-11-09 会话内复跑）。大量 P2 功能变更**尚未 git 提交**（见变更记录），与 [`phase1.md`](phase1.md) 当时「证据先行」口径一致。

**签收会话**：2026-11-09（P2-27 编排）。

## 验收清单

| # | 验收标准 | 命令 / 测试 | 证据 |
|---|---|---|---|
| 1 | **§8.3 / 阶段 2**：含无效变量引用的 AI 产出被闸门拦截，Issue 定位到节点/字段 | `cargo test -p generator --test gate_feedback_cassette` | **通过**：`cassette_gate_loop_corrects_invalid_variable_reference`（`reference.not_ancestor` → 节点 `a`、headers）；`cassette_gate_loop_needs_manual_review_when_still_invalid`；`gate_feedback_json_lists_errors_only` — 3 passed（1 ignored 辅助录制），2026-11-09 |
| 2 | **P2-18**：闸门回喂后修正或标记「需人工介入」 | 同上 + `cargo test -p generator --test repair_cassette` | **通过**：`cassette_repair_fixes_broken_pay_reference`；修复提示含 gate 错误 — 2 passed，2026-11-09 |
| 3 | **P2-23**：UI「交给 AI 修正」恢复校验通过并写入待审 | `cargo test -p server --test dag_repair_api`；`cd web && npx playwright test e2e/gate-ai-repair.spec.ts` | **HTTP**：`repair_ai_returns_pending_candidate_on_success` — 1 passed。**E2E**：`gate failure offers AI repair and restores passing validation` — 1 passed (~0.25s)，2026-11-09（本机 chromium 已装） |
| 4 | **§8.3 / 阶段 2**：激活后 AI DAG 与手工 DAG 在 mock-sut 上确定性报告一致 | `cargo test -p cli --test ai_serial_consistency` | **通过**：`activated_ai_serial_matches_manual_example_on_mock_sut` — 1 passed（~0.15s），2026-11-09 |
| 5 | **P2-20 / P2-21**：AI 候选仅 `pending`，激活必经闸门 | `cargo test -p server --test pending_ai`；`cargo test -p cli --test dag_store pending` | **通过**：`ai_candidate_save_list_detail_and_requires_gate_to_activate`、`ai_candidate_save_never_sets_active`；CLI `pending_lists_only_ai_review_queue`、`activate_runs_calibration_then_gate` — 2026-11-09 |
| 6 | **P2-22**：待审预览、diff、激活 | `npx playwright test e2e/pending-ai.spec.ts` | **通过**：`ai pending preview diff and activate` — 1 passed (~0.18s)，2026-11-09 |
| 7 | **P2-13**：导入预览、低置信高亮、合并提交 | `cargo test -p cli --test api_import`；`npx playwright test e2e/api-import.spec.ts` | **CLI**：五格式 import 草稿 — workspace test 绿。**E2E**：`import preview highlights…` — 1 passed (~0.17s)，2026-11-09 |
| 8 | **P2-25**：生成闸门接受率 harness（目标 ≥70%） | `./scripts/generation-eval.sh`；`cargo test -p generator --test eval_harness` | **通过**：[`generation-eval.md`](./generation-eval.md) 3/3 首轮+终轮 PASS；`cassette_eval_meets_gate_acceptance_target` — 2026-11-09 |
| 9 | **P2-27 收尾**：全量检查 | `./scripts/check.sh` | **通过（exit 0）**：`cargo fmt` / clippy / `cargo test` / `web: vitest` / 类型生成 / build / 红线守卫 — 墙钟 ~17–65s（会话内两次全绿），2026-11-09 |
| 10 | **CI** | `.github/workflows/ci.yml` → `web-e2e` | Playwright：**`pending-ai`**、**`gate-ai-repair`**、**`api-import`** 与阶段 1 E2E 同一 job |

## 与设计对齐的补充说明

| 主题 | 状态 |
|---|---|
| AI 产物不经闸门直接 `active` | **已禁止**：`store` + `server` + CLI 测试覆盖 |
| 默认构建无 `llm-gateway`（红线） | **保持**：`check.sh` 架构守卫 |
| LLM 默认 Mock / Cassette | **是**；真实调用仅 `AITEST_LIVE_LLM=1`（**P2-02**） |
| M0 Q3 ≥70% 接受率 | **cassette harness 达标**；真实模型需运维环境复跑 `generation-eval.sh` |

## 遗留问题

| 项 | 说明 | 跟进 |
|---|---|---|
| **P2-24** AI 副驾 Tab | 阶段 2 任务表内仍为 `TODO`，非 P2-27 依赖 | 阶段 3 并行或单独认领 |
| 未提交变更 | P2-13/21/23/25/26 等大量本地 `DONE` 未 commit | 人类指示后分批提交 |
| 本机 Playwright | 已安装 chromium-headless-shell v1254（2026-11-09） | 新环境复现：`cd web && npx playwright install chromium` |
| 接受率与真实 LLM | 当前报告为 **cassette** 确定性回放 | 上线前用 `AITEST_LIVE_LLM=1` 写一版 `generation-eval.md`（可选） |

## 交叉引用

- 阶段 0：[`phase0.md`](./phase0.md)
- 阶段 1：[`phase1.md`](./phase1.md)
- M0 Q3 接受率：[`generation-eval.md`](./generation-eval.md)、[`m0.md`](./m0.md) Q3 行
