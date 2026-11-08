# ADR-003：运行取消的节点状态

- 状态：已采纳
- 任务：P0-39
- 日期：2026-11-08

## 背景

`develop.md` 中 `dag_runs.status` 包含 `CANCELLED`，但 §3.2 的节点状态机没有覆盖取消：`RUNNING`/`POLLING` 只能进入 `COMPLETED` 或 `FAILED`。P0-39 要求取消时让运行中的节点尽快终止，同时 cleanup 仍然执行。另外，每个 `FAILED` 节点都必须有确定性的 `failure_category`（红线 6），而 §4.5 的分类枚举里没有"被取消"。

## 选项

1. 新增节点状态 `CANCELLED`。需要迁移重建 `node_executions` 表（SQLite 无法修改 CHECK 约束），前端和报告也要多处理一种状态。
2. 被中断的节点记为 `FAILED`，并新增一个错误类别。这会让取消进入分类器和自主性策略，可能触发重试或 LLM 修复，语义上不对。
3. 允许 `RUNNING`/`POLLING -> SKIPPED`，被中断的节点记为 `SKIPPED`，并附带 `cancelled` 错误；运行级状态为 `CANCELLED`。

## 决定

采用选项 3。

- `model::NodeStatus` 新增 `RUNNING -> SKIPPED` 和 `POLLING -> SKIPPED` 两条迁移；`SKIPPED` 原本的含义"未执行（上游失败或运行取消）"扩展为"未执行完"。
- 取消令牌 `CancelToken` 由调用方持有，通过 `Scheduler::with_cancel` 和 `ExecContext.cancel` 传递，子工作流把同一个令牌交给子 DAG。
- 主阶段收到取消后：未启动的非 cleanup 节点记为 `SKIPPED`（不带 error）；运行中的节点 future 被丢弃（HTTP 请求、退避等待随之中止），记为 `SKIPPED`，error 为 `ExecErrorKind::Cancelled`。
- 子工作流节点不会被直接中断，而是由子 DAG 观察同一个令牌自行收尾，所以子 DAG 的 cleanup 也会执行；子 DAG 因取消结束时，子工作流节点返回 `Cancelled` 错误，同样记为 `SKIPPED`。
- cleanup 阶段不受令牌影响。只有主阶段内收到的取消才会把运行记为 `CANCELLED`（`RunOutcome::status()`）。
- 取消前已经得出的结果保持不变：`COMPLETED` 仍是 `COMPLETED`，`FAILED` 仍是 `FAILED`。

## 后果

- 不需要迁移；`FAILED` 仍然只表示真实失败，分类器和自主性策略不会看到取消。
- 状态机比 §3.2 宽松：调度器出 bug 时，运行中的节点也可能被错误地标为 `SKIPPED`。判断节点是否被中断，要看 `NodeResult.error.kind == cancelled`，或者看它是否出现在 `started` 中。
- P0-40 持久化时，`SKIPPED` 行可能带有 `started_at`。
