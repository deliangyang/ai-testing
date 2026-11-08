# 规范化运行报告

`mytool run --report-json <path>` 写出的文件。整份文件是一份规范化 JSON（键按 UTF-8 字节序排序、无多余空白、浮点数用 RFC 8785 / ECMAScript 格式，与 `model::to_canonical_json` 相同）。

文档形状由 `schemas/run-report.schema.json` 约束，与 `cli::report::RunReport` 同步；更新命令：`UPDATE_SCHEMAS=1 cargo test -p cli --test report_schema`。

`schema` 字段为 `aitest.run-report/1`。形状变了就加版本号，不改已发布版本的含义。

## 分区

| 分区 | 内容 | 稳定性 |
|---|---|---|
| `deterministic` | DAG id/版本、环境名、运行终态、`--no-llm`、每个节点的状态、去掉时钟后的输出、断言逐条结果、轮询轨迹（无时间戳）、失败分类 | 同一已激活 DAG + 同一环境，两次 `--no-llm` 运行，该对象规范化后逐字节一致 |
| `volatile` | `run_id`、运行起止时间、每个节点的 `timing.duration_ms`、响应里会变的头（`date`/`expires`/`age`/`retry-after`）、每轮轮询的 `ts` | 每次运行都不同 |

比较两次运行是否确定，只比 `deterministic`，不要比整份文件。

## `deterministic.nodes`

键是节点 id（含子工作流帧路径，如 `login/session`），按 id 排序。每项：

| 字段 | 何时出现 |
|---|---|
| `status` | 总是。取值 `PENDING`…`SKIPPED`，与引擎状态机一致 |
| `output` | 节点有输出时。http/poll 为 `{status, headers, body}`（无 `timing`，无 Date 类头） |
| `assertions` | assert 节点，或失败时的 `error.detail.results`。每条 `{path, operator, expected?, actual?, passed, message?}` |
| `poll` | poll 节点的迭代轨迹，无 `ts` |
| `error` | 节点带执行错误时。`{kind, message, detail?}`，`kind` 为 snake_case |
| `failure` | 仅 `FAILED`：`{category, error_code}`，由确定性分类器给出 |

凭据已经过环境脱敏，报告里只会出现 `****`。

## `volatile.nodes`

只为真正带有易变字段的节点出现：

| 字段 | 来源 |
|---|---|
| `timing` | http/poll 输出的 `{duration_ms}` |
| `headers` | 从确定性输出里拿走的 Date 类头 |
| `poll` | `[{attempt, ts}, ...]`，与确定性轨迹按 attempt 对应 |
