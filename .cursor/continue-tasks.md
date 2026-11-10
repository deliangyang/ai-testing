# 连续推进任务

把下面这段发给主会话。`task-implementer` 每次只做一项，连续推进由主会话一轮一轮调用它。

```text
按 AGENTS.md 连续推进 TASKS.md，做到没有可做任务为止。

规则：
- 每轮只让 task-implementer 做一个任务：有 DOING 就续做，否则选依赖全部 DONE、阶段最小、序号最小的 TODO。
- 它返回后立刻开始下一个，不要停下来问我要不要继续。
- 每个任务在 ./scripts/check.ps1 全绿后提交并推送到当前分支。提交信息用 [任务ID] 开头。TASKS.md 的状态和变更记录与代码同一提交。
- 一次只做一个任务的范围：缺前置就先插入并只做前置；任务过大就拆开，从第一片做起。
- 每轮 task-implementer 返回后，主会话只保留四行：任务 ID、提交哈希、check.ps1 是否全绿、下一个任务 ID。不复述 diff、测试日志、文件内容。
- 不在主会话里读 develop.md 或实现代码；这些只由 task-implementer 读。
- 若发现更早的对话已被压缩，先重读 TASKS.md 的状态列和 .cursor/continue-tasks.md，再开始下一轮。

只有这些情况才停下来报告，不要自行绕过：
- 没有依赖已满足的 TODO
- 任务是 BLOCKED，或要登记待决问题且没有书面默认值
- check.ps1 修不到全绿
- 红线被打破（确定性路径依赖模型、裸 sleep、AI 产物直接 active、凭据入库或进模型）
```
