# ADR-004：运行证据的持久化方式

- 状态：已采纳
- 任务：P0-40
- 日期：2026-11-08

## 背景

P0-40 要求运行全过程写入 `dag_runs`/`node_executions`（含 input_resolved、output、poll_trace），并能从库中完整重建每个节点的证据。设计没有规定三件事：引擎如何与 store 交互，失败节点的错误详情存在哪里，以及分类器（P0-43/P0-44）接入之前，`FAILED` 行必需的分类从哪里来。

## 决定

- **记录钩子**：调度器通过 `RunRecorder` trait 报告两类事件：`Started`（进入 RUNNING，附带渲染后的节点配置）和 `Finished`（进入终态，附带 `NodeResult`）。调用放在 `spawn_blocking` 里，同一调度器内按观察到的顺序串行执行。`StoreRecorder` 写 SQLite；CLI/UI 以后可以换成别的实现。
- **子工作流**：记录器经 `ExecContext.recorder` 传给子调度器，子节点以帧路径（ADR-002）作为 node_id 直接写入。父调度器不会重复记录子节点。
- **行的生命周期**：节点启动时插入 RUNNING 行，`input_resolved` 为渲染后的节点配置；渲染失败时为 NULL，节点随后以 template 错误失败。没有启动就被跳过的节点，在终态时补插 PENDING 行，再立即更新为 SKIPPED。READY/POLLING 这类瞬时状态不落库，实时状态由 P0-41 的事件流提供。attempt 暂时固定为 1。
- **output_json 的含义**：COMPLETED 行存节点输出；以错误结束的行（FAILED，或被取消中断的 SKIPPED）存序列化后的 `ExecError`（不含 children 和 trace，trace 在 `poll_trace_json` 中）。`load_results` 按这个约定重建出的结果与 `RunOutcome.nodes` 相等。
- **分类**：`FAILED` 行的分类经 `FailureClassifier` trait 得出。P0-44 之前使用占位实现 `Unclassified`（`unknown` / `UNCLASSIFIED`）；P0-44 替换为规则分类器，并同时写 `failure_records`。
- **脱敏**：记录前，调度器把 input、output、trace、错误消息和 detail 中出现的来自进程环境变量的值替换为 `****`（`Environment::mask`），任何 `RunRecorder` 实现拿到的都是已脱敏的数据（红线 5）。内存中的 `RunOutcome` 不脱敏，执行器仍需要使用真实值。
- **记录失败**：记录失败不会中止运行（cleanup 仍须执行），但从第一次失败起不再记录，`Scheduler::run` 最终返回 `SchedulerError::Record`。
- **运行行**：`StoreRecorder::begin_run` 创建运行并把它置为 RUNNING，`finish_run` 按 `RunOutcome::status()` 写入终态。由调用方（CLI/server）在调度前后调用。

## 后果

- 重建证据不需要额外的列或迁移。读取方必须按状态区分 `output_json` 是输出还是错误。
- 脱敏基于子串替换：值很短时可能误伤无关文本，但不会漏掉原文。
- P0-44 之前，失败行的分类都是 `unknown`。
