# Phase 1 acceptance (`develop.md` §8.3)

**阶段验收摘录（[`TASKS.md`](../../TASKS.md) 阶段 1，§5 段首）**：排查一次轮询超时失败，点失败节点右面板即显示完整轨迹和断言 diff。

设计对照：[`develop.md`](../../develop.md) **§7.1** 阶段 1 里程碑表「验收」列、**§8.3** 分阶段验收阶段 1 行（与上句同义）；M0 两周联合评审 **§7.3** 问题 2（线 B：失败变红 → 右面板完整证据是否在 2 次点击内）。

任务 **P1-32**（阶段 1 验收）依赖 P1-29、P1-25、P1-24、P1-31；本文件为验收预备稿，正式逐条复核在 P1-32 会话完成。

**结论**：**待定（TBD）**。待 **P1-31**（nsh 外壳集成）标 `DONE` 后，由 P1-32 填写结论并补全「证据」列。

## 验收清单

| # | 验收标准 | 命令 / 测试 | 证据 |
|---|---|---|---|
| 1 | **§8.3 / §7.1 阶段 1**：轮询超时失败后，点失败节点，右面板展示完整轮询轨迹与断言 diff | `cd web; npx playwright test e2e/failure-evidence.spec.ts`（`poll timeout failure shows full evidence within two clicks`） | **Web 目标已就绪**（P1-29）；P1-32 正式签收时再跑并记录输出 |
| 2 | **§7.3 问题 2（线 B）**：从失败节点「变红」到人在右面板看到完整证据 ≤2 次点击（P1-29） | 同上 | 同 #1；可与 `docs/reviews/m0.md` Q2 草稿对齐 |
| 3 | **P1-25**：报告回放视图——最终态叠加、摘要条、时间轴滑块回放 | `cd web; npm test -- DagReportReplayView` | P1-25 DONE；正式验收时附 vitest 通过记录 |
| 4 | **P1-24**：编辑时实时校验 API；徽标 ✓/⚠/✗；问题列表点击定位节点 | `cd web; npx playwright test e2e/gate-validation.spec.ts`（`live gate validation updates badge and focuses issue node`） | P1-24 DONE；正式验收时附 E2E 通过记录 |
| 5 | **P1-29**：Playwright E2E（轮询超时证据链）纳入仓库质量检查 | `.github/workflows/ci.yml` job `web-e2e`；本地 `./scripts/check.ps1`（web 段） | CI 已配置；全量 `check.ps1` 留 P1-32 收尾 (#7) |
| 6 | **P1-31**：nsh 外壳以 `native_module` 实现 `DataSource`，复用全部 web 组件；`nsh test` 跑通核心用例 | `cargo test -p nsh-bridge`；`nsh test app/aitest`（或 `scripts/nsh-spike.ps1`） | pending P1-31 |
| 7 | P1-32 收尾：全量检查与 E2E 在同一变更集内全绿 | `./scripts/check.ps1` | pending P1-31 |

## 遗留问题
