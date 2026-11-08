# Phase 1 acceptance (`develop.md` §8.3)

**阶段验收摘录（[`TASKS.md`](../../TASKS.md) 阶段 1，§5 段首）**：排查一次轮询超时失败，点失败节点右面板即显示完整轨迹和断言 diff。

设计对照：[`develop.md`](../../develop.md) **§7.1** 阶段 1 里程碑表「验收」列、**§8.3** 分阶段验收阶段 1 行（与上句同义）；M0 两周联合评审 **§7.3** 问题 2（线 B：失败变红 → 右面板完整证据是否在 2 次点击内）。

任务 **P1-32**（阶段 1 验收）依赖 P1-29、P1-25、P1-24、P1-31。

**结论**：**不通过**。阶段 1 **功能验收项**（#1–#6）关键 E2E/Vitest/`nsh-bridge` 在本机会话已复跑通过；**#7 全量 `./scripts/check.ps1` 未绿**（prettier / clippy / eslint 先期失败），P1-32 保持 `DOING`，修复见 **P1-32-fix1**。

**签收会话**：2026-11-09（P1-32）。

## 验收清单

| # | 验收标准 | 命令 / 测试 | 证据 |
|---|---|---|---|
| 1 | **§8.3 / §7.1 阶段 1**：轮询超时失败后，点失败节点，右面板展示完整轮询轨迹与断言 diff | `cd web; npx playwright test e2e/failure-evidence.spec.ts` | **通过**：`poll timeout failure shows full evidence within two clicks` — 1 passed (~27.2s)，2026-11-09 |
| 2 | **§7.3 问题 2（线 B）**：从失败节点「变红」到人在右面板看到完整证据 ≤2 次点击（P1-29） | 同上 | 同 #1；与 [`docs/reviews/m0.md`](m0.md) Q2 线 B 一致 |
| 3 | **P1-25**：报告回放视图——最终态叠加、摘要条、时间轴滑块回放 | `cd web; npm test -- DagReportReplayView` | **通过**：`DagReportReplayView.test.tsx` 1 passed（首次 default forks worker 超时 60s；重跑 `npx vitest run DagReportReplayView --pool=threads` 21.5s 通过），2026-11-09 |
| 4 | **P1-24**：编辑时实时校验 API；徽标 ✓/⚠/✗；问题列表点击定位节点 | `cd web; npx playwright test e2e/gate-validation.spec.ts` | **通过**：`live gate validation updates badge and focuses issue node` — 1 passed (~14.9s)，2026-11-09 |
| 5 | **P1-29**：Playwright E2E（轮询超时证据链）纳入仓库质量检查 | `.github/workflows/ci.yml` job `web-e2e` | **已配置**：`ci.yml` → `web-e2e`（`ubuntu-latest`，`npm ci` + Playwright）；与 #1 本地 E2E 一致 |
| 6 | **P1-31**：nsh 外壳以 `native_module` 实现 `DataSource`，复用全部 web 组件；`nsh test` 跑通核心用例 | `cargo test -p nsh-bridge`；`nsh test app/aitest` | **`cargo test -p nsh-bridge`**：2 项集成测试通过（`bridge.rs`、`datasource.rs`），2026-11-09。**`nsh test app/aitest`**：**SKIP** — 本机 `nsh` 不在 PATH（与 P1-31 变更记录一致）；探针以 `nsh-bridge` 单测 + `scripts/nsh-spike.ps1` 为准 |
| 7 | P1-32 收尾：全量检查与 E2E 在同一变更集内全绿 | `./scripts/check.ps1` | **未通过（exit 1）**：2026-11-09 单次 `./scripts/check.ps1` — 失败步骤 **`web: prettier`**（34 文件 format:check）、**`cargo clippy`**（`crates/server/tests/sse.rs:87` `expect_used`×3）、**`web: eslint`**（35 problems：e2e/playwright 未入 `parserOptions.project`、多条 `react-hooks`/`@typescript-eslint`）。`cargo test` 段在 `cli` golden 用例上长时间运行（会话内 >25min 未见后续 `web: vitest` 段） |

## 遗留问题

| 项 | 说明 | 跟进 |
|---|---|---|
| 质量门未绿 | `check.ps1` 因 prettier / clippy / eslint 失败，不满足 P1-32 验收标准 #7 与 AGENTS.md DoD | **P1-32-fix1** |
| Vitest forks 超时 | 默认 `npm test -- DagReportReplayView` 出现 vitest-pool fork worker 60s 超时；`--pool=threads` 可通过 | 纳入 fix1（确认 CI/`check.ps1` web:vitest 稳定） |
| `nsh` CLI 缺省 | 本机无 `nsh`，无法签收 `nsh test app/aitest` | 文档已 SKIP；有 CLI 的环境补跑并回写证据（可选） |
| Golden 集成测耗时 | 全量 `cargo test` 在 `cli/tests/golden.rs` 上极慢，阻塞 `check.ps1` 后续 web 步骤 | fix1 中确认非挂死并记录 wall time |
