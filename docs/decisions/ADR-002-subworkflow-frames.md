# ADR-002：子工作流帧路径与子 DAG 加载规则

- 状态：已采纳
- 任务：P0-37
- 日期：2026-11-08

## 背景

`develop.md` §4.4 要求子工作流通过帧栈实现、变量作用域隔离、输入输出通过显式 binding 映射，并且记录帧路径，但没有规定两件事：子节点的执行结果如何在报告和 `node_executions` 中标识，以及按 `dag_ref` 加载时哪些版本可以运行。`node_executions` 只有 `node_id` 列，没有帧相关的列。

## 选项

1. 新增迁移，给 `node_executions` 加 `frame_path` 列。
2. 把帧路径编码进 node_id：`<子工作流节点>/<子节点>`，嵌套时继续拼接。
3. 子节点结果只放在子工作流节点的输出里，不单独记录。

## 决定

采用选项 2。

- 合法的节点 id 只含 `[A-Za-z0-9_-]`（闸门 structure 规则），因此 `/` 不会与真实 id 冲突，路径可以无歧义地拆分。
- `RunOutcome.nodes`、确定性报告以及 P0-40 写入的 `node_executions.node_id` 都使用这个路径，例如 `buy/login/token`。
- 每层帧记作 `Frame{node, dag, version}`，通过 `ExecContext.frames` 传给子节点（从外到内）。
- 子 DAG 只能看到渲染后的 `inputs`（保留节点 `input`）和环境；`outputs` 在子 DAG 成功后按子作用域渲染，父 DAG 只能看到这些输出。
- 加载规则：没有 pin 版本时取 active 版本；pin 了版本时只接受 `active` 或 `archived`（即激活过的版本），`pending` 版本拒绝运行，满足红线 4「AI 产物永不直接生效」。
- 运行时如果帧栈里出现重复的 DAG id，就以 `subworkflow cycle` 失败。这是对 P0-38 保存时环检测的兜底。

## 后果

- 不需要新迁移；按前缀查询（`node_id LIKE 'buy/%'`）即可取出某个帧下的全部证据。
- 节点 id 的字符集约束成为持久化格式的一部分，以后放宽时必须同时修改这里。
- 子 DAG 默认串行执行（`SubworkflowExecutor::with_max_parallel` 可调），环境沿用父运行的环境。
