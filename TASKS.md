# AI 测试系统 · 开发任务表

> 设计依据：[`develop.md`](./develop.md)（下文以 `§x.y` 引用章节）。推进规则见 [`AGENTS.md`](./AGENTS.md)。
> 本文件是**开发进度的唯一事实来源**。每完成/阻塞一个任务，必须同步更新状态并追加变更记录。

## 状态说明

| 状态 | 含义 |
|---|---|
| `TODO` | 未开始 |
| `DOING` | 进行中（同一时刻最多 1 个） |
| `DONE` | 验收标准全部满足，检查脚本通过，已提交 |
| `BLOCKED` | 被待决问题或外部条件阻塞，须在「待决问题」登记原因 |

任务 ID 规则：`P<阶段>-<序号>`。任务过大需拆分时使用后缀，如 `P0-33a`、`P0-33b`，原任务改为两者的汇总项。

## 起跑线映射（§7.2，前 2 周三线并行）

| 线 | 目标 | 任务 |
|---|---|---|
| A 引擎语义 spike | http/poll/assert + 变量绑定 + 分类器跑通 3 条真实链路 | P0-01 ~ P0-11、P0-21 ~ P0-35、P0-40 ~ P0-55 |
| B UI 骨架（Web 目标） | 画布三模式 + 右面板 + 轮询轨迹，读线 A 的数据 | P1-01、P1-06 ~ P1-13（先用 fixture 数据源，不等后端） |
| C 闸门原型 | 20 个坏 DAG 100% 拦截 | P0-13 ~ P0-20 |

M0 两周联合评审：P0-56。

---

## 阶段 0：骨架与引擎（§7.1，4–6 周）

阶段验收：同一 DAG 两次 `--no-llm` 执行断言逐字节一致，无 LLM 调用；轮询超时轨迹完整可回放。

### 0.1 工程骨架

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P0-01 | 初始化 git 与 Cargo workspace：创建空 crate `common`/`model`/`gate`/`store`/`classifier`/`engine`/`cli`；`rust-toolchain.toml`、`.gitignore`、`.editorconfig`、`rustfmt.toml` | `/`、`crates/` | — | `cargo build --workspace` 通过；目录与 AGENTS.md「仓库布局」一致 | DONE |
| P0-02 | 质量检查脚本 `scripts/check.ps1`（及 `check.sh`）：fmt --check、clippy `-D warnings`、test；内置架构守卫（见 AGENTS.md「红线」第 1、2 条） | `scripts/` | P0-01 | 干净仓库退出码 0；人为在 engine 中写 `tokio::time::sleep` 或依赖 llm 类 crate 时退出码非 0 | DONE |
| P0-02-fix1 | `check.ps1` 漏报 fmt/test 失败：原生命令的 stdout 混进 `Invoke-Native` 返回值，非空数组恒为真 | `scripts/` | P0-02 | 未格式化文件使 check.ps1 退出码非 0；格式化后为 0 | DONE |
| P0-03 | CI 流水线 `.github/workflows/ci.yml`，Windows + Linux 矩阵调用检查脚本 | `.github/` | P0-02 | 推送后流水线绿 | DOING |
| P0-04 | `common` crate：错误约定（thiserror）、tracing 初始化、`Clock`/`Sleeper`/`IdGen` trait 及真实实现与固定实现（`FixedClock`、`ManualSleeper`、`SeqIdGen`） | `crates/common` | P0-01 | 单测覆盖固定实现可推进时间、顺序生成 ID | DONE |

### 0.2 DAG 模型（§4.1）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P0-05 | DAG 顶层类型：`Dag{id,version,nodes,edges,meta}`、`Node{id,name,kind}`、`Edge{from,to}`、`NodeKind` 枚举（六种），serde JSON/YAML 双向 | `crates/model` | P0-01 | JSON↔YAML round-trip 测试；未知节点类型反序列化报错并带路径 | DONE |
| P0-06 | `http` 节点配置：method、path、query、headers、body 模板、`response_schema`（可选，JSON Schema） | `crates/model` | P0-05 | 样例 YAML 反序列化成功；非法 method 报错 | DONE |
| P0-07 | `poll` 节点配置：被轮询请求、结构化 `condition`（path + op + expected，不允许自由表达式）、`backoff`（间隔序列，ms）、`timeout_ms`；缺 backoff/timeout 反序列化即失败 | `crates/model` | P0-06 | 缺字段用例失败；合法用例通过 | DONE |
| P0-08 | `assert` 节点配置：`target`（引用上游输出）、`path`、`operator` 枚举（eq/ne/gt/gte/lt/lte/contains/exists/not_exists/regex/len_eq/type_is）、`expected` | `crates/model` | P0-05 | 每种 operator 均有反序列化用例 | DONE |
| P0-09 | `db_query`/`subworkflow`/`cleanup` 配置类型（仅结构，执行在后续任务）：连接名+参数化 SQL+结果映射；`dag_ref{id,version_req}`+输入/输出 binding；cleanup 动作列表 | `crates/model` | P0-05 | 反序列化用例通过 | DONE |
| P0-10 | 变量引用解析器：`${node.output.a.b[0]}`、`${env:VAR}`、`${auth.name}`、`$${` 转义；输出 AST，可从任意模板中提取全部引用 | `crates/model` | P0-05 | 表驱动单测 ≥20 例，含嵌套数组、非法语法定位到字符偏移 | DONE |
| P0-11 | 规范化 JSON（键排序、数字规范化、无多余空白）与内容哈希（sha256） | `crates/model` | P0-05 | 键顺序不同的等价对象哈希相同；属性测试（proptest） | DONE |
| P0-12 | 由类型导出 JSON Schema（schemars）到 `schemas/dag.schema.json`；测试校验文件与代码同步 | `crates/model`、`schemas/` | P0-09 | 修改类型不更新 schema 时测试失败 | DONE |

### 0.3 确定性校验闸门（§4.3，线 C）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P0-13 | 闸门框架：`Rule` trait、`Issue{code,severity(error/warn),node_id,json_path,message}`、`validate(&Dag)->Report`；规则可注册、结果按 (node_id, code) 稳定排序 | `crates/gate` | P0-05 | 空 DAG 与合法 DAG 用例；输出顺序确定 | DONE |
| P0-14 | 结构规则：节点 ID 唯一、边端点存在、禁止自环、禁止重复边、禁止使用保留 id（`input`/`env`/`auth`） | `crates/gate` | P0-13 | 每条规则至少一个正例一个反例 | DONE |
| P0-15 | 无环规则：Kahn 拓扑排序，失败时报告一条具体环路径 | `crates/gate` | P0-13 | 环 a→b→c→a 报告完整路径 | DONE |
| P0-16 | 前驱引用规则：模板中 `${x.output...}` 的 x 必须是当前节点的祖先（传递闭包） | `crates/gate` | P0-10、P0-15 | 引用兄弟/后继节点均被拦截并定位到字段 | DONE |
| P0-17 | 字段存在规则：被引用字段路径必须在上游 `response_schema` 中存在；上游无 schema 时降级为 warn | `crates/gate` | P0-16、P0-06 | 不存在字段报 error；无 schema 报 warn | DONE |
| P0-18 | 轮询可判定规则：condition 路径可解析、op 与 expected 类型匹配、backoff 非空且单调非减、timeout ≥ backoff 首项 | `crates/gate` | P0-07、P0-13 | 4 类违规各一例 | DONE |
| P0-19 | 完整性规则：必填字段、孤立节点（无边且非唯一节点）、cleanup 必须无出边、每个 http 节点至少被一个 assert 引用（warn） | `crates/gate` | P0-13 | 各违规一例 | DONE |
| P0-20 | 坏 DAG 语料：`tests/fixtures/bad_dags/` ≥20 个（环/无效引用/轮询不可判定/孤立节点/缺字段…），每个文件头注明期望 issue code；全量拦截测试 | `crates/gate/tests` | P0-14 ~ P0-19 | 100% 被拦截且命中期望 code | DONE |

### 0.4 持久化（§3.1，§2.2）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P0-21 | SQLite 连接层：WAL、foreign_keys、busy_timeout；嵌入式 SQL 迁移框架（`migrations/V###__name.sql` + 版本表） | `crates/store` | P0-01 | 新库自动迁移；重复启动幂等 | DONE |
| P0-22 | 迁移 V001：`api_specs`、`dag_definitions`、`dag_runs`、`node_executions`、`failure_records`（按 §3.1，补必要索引） | `crates/store/migrations` | P0-21 | 迁移后 schema 快照测试 | DONE |
| P0-23 | `DagRepository`：保存新版本（pending）、激活（同 id 仅一个 active，旧版 archived）、按版本/取 active/列版本 | `crates/store` | P0-22、P0-05 | 并发激活只有一个成功；旧版本仍可读取 | DONE |
| P0-24 | `RunRepository`：创建运行、状态迁移（非法迁移返回错误）、查询 | `crates/store` | P0-22 | 状态迁移矩阵表驱动测试 | DONE |
| P0-25 | `NodeExecutionRepository`：按 (run, node, attempt) 写入；终态行不可再修改 | `crates/store` | P0-22 | 修改终态行返回错误 | DONE |
| P0-26 | `FailureRecordRepository`：写入与按 run/category 查询 | `crates/store` | P0-22 | 单测 | TODO |

### 0.5 确定性执行引擎（§4.2，§3.2）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P0-27 | 节点状态机：PENDING/READY/RUNNING/POLLING/COMPLETED/FAILED/SKIPPED 及合法迁移 | `crates/engine` | P0-01 | 迁移矩阵全覆盖测试 | TODO |
| P0-28 | `NodeExecutor` trait + 串行拓扑调度器（用 mock 执行器测试） | `crates/engine` | P0-27、P0-15 | 执行顺序符合拓扑序；同层按 node_id 字典序 | TODO |
| P0-29 | 并行调度：无依赖后继同时发起，有界信号量控制并行度；**结果写入与报告顺序与完成时序无关** | `crates/engine` | P0-28 | c∥d 并行用例：并行度 1 与 4 的报告字节一致 | TODO |
| P0-30 | 失败传播：节点 FAILED 后所有未执行后继标记 SKIPPED（cleanup 除外） | `crates/engine` | P0-28 | 菱形 DAG 用例 | TODO |
| P0-31 | 环境配置加载：`environments/<name>.yaml`（base_url、变量、`${env:VAR}` 引用）；缺失变量报明确错误 | `crates/engine` | P0-10 | 单测 | TODO |
| P0-32 | 运行时变量上下文与模板渲染：按引用 AST 解析上游输出/环境变量，产出 `input_resolved_json` | `crates/engine` | P0-10、P0-31 | 引用缺失时节点失败并带路径；解析结果可序列化 | TODO |
| P0-33 | HTTP 执行器（reqwest）：渲染请求、超时、输出 `{status,headers,body,timing}`；wiremock 测试 | `crates/engine` | P0-32、P0-06 | GET/POST/JSON body/超时 用例 | TODO |
| P0-34 | JSONPath 取值工具 + Assert 执行器：逐条断言结果含 `{path,operator,expected,actual,passed}` diff | `crates/engine` | P0-08、P0-32 | 每种 operator 正反例 | TODO |
| P0-35 | Poll 执行器：按 backoff 序列迭代（末项重复）、总超时；每次迭代写 trace `{ts,attempt,actual,expected,diff}`；**只能通过 `Sleeper` 等待**；超时保留完整轨迹 | `crates/engine` | P0-04、P0-33、P0-07 | 用 ManualSleeper 测试成功/超时，轨迹条数与时间戳精确 | TODO |
| P0-36 | cleanup 节点：DAG 终态无条件执行（前序成功/失败/取消均执行） | `crates/engine` | P0-30 | 三种前序结果各一例 | TODO |
| P0-37 | 子工作流帧栈：按 `dag_ref` 从 store 加载子 DAG，变量作用域隔离，仅通过显式 binding 映射输入/输出；记录帧路径 | `crates/engine` | P0-28、P0-23、P0-09 | 子 DAG 无法读取父变量；输出映射正确；两层嵌套用例 | TODO |
| P0-38 | 闸门规则：跨 DAG 子工作流引用环检测（保存时执行，需要 store 只读视图） | `crates/gate` | P0-37 | A→B→A 引用环被拦截 | TODO |
| P0-39 | 运行取消：取消令牌，运行中节点尽快终止，状态 CANCELLED，cleanup 仍执行 | `crates/engine` | P0-36 | 单测 | TODO |
| P0-40 | 持久化接入：运行全过程写 `dag_runs`/`node_executions`（含 input_resolved、output、poll_trace） | `crates/engine` | P0-24、P0-25、P0-35 | 运行后可从库中完整重建每个节点证据 | TODO |
| P0-41 | 引擎事件流 `EngineEvent`（NodeStateChanged/PollIteration/Log/RunFinished），通过 channel 输出，供 CLI 与 UI 订阅 | `crates/engine` | P0-28 | 事件顺序测试 | TODO |

### 0.6 失败分类器（§4.5）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P0-42 | `FailureCategory` 枚举（7 类）与 `error_code` 目录，文档化到 `docs/error-codes.md` | `crates/classifier`、`docs/` | P0-01 | 枚举与文档一一对应（测试比对） | TODO |
| P0-43 | 确定性分类器：输入（执行器错误类型、HTTP 状态、断言结果、提取错误、原始消息）→ (category, error_code)；规则表驱动（正则 + 状态码 + 错误类型），无 LLM | `crates/classifier` | P0-42 | 每个 category ≥3 个样本测试；无法匹配归 `unknown` | TODO |
| P0-44 | 引擎接入分类器：失败节点写 `failure_category`/`error_code` 与 `failure_records` | `crates/engine` | P0-43、P0-40、P0-26 | 连接拒绝→connection_error；状态码不符→assertion_failed_status 等端到端用例 | TODO |

### 0.7 CLI（§4.12，阶段 0 子集）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P0-45 | CLI 骨架（clap）：二进制 `mytool`，全局 `--db <path>`、`--log-level`；子命令分组 `dag`/`run` | `crates/cli` | P0-01 | `mytool --help` 输出子命令 | TODO |
| P0-46 | `mytool dag validate <file>`：输出闸门 Report（文本/JSON），有 error 时退出码 3 | `crates/cli` | P0-45、P0-20 | 坏 DAG 语料全部非 0 退出 | TODO |
| P0-47 | `mytool dag save <file>` / `dag activate <id> <version>` / `dag list`；save 前强制闸门 | `crates/cli` | P0-46、P0-23 | 未通过闸门的文件无法保存 | TODO |
| P0-48 | `mytool run --dag <id\|file> --env <name> --no-llm`：调用引擎，实时打印节点进度，退出码反映结果 | `crates/cli` | P0-47、P0-44、P0-41 | 三条样例链路可运行 | TODO |
| P0-49 | 规范化运行报告 `--report-json <path>`：确定性部分（节点状态、断言结果、分类）与易变部分（时间戳、耗时）分区；确定性部分规范化输出 | `crates/cli` | P0-48、P0-11 | 报告 schema 文档化 | TODO |
| P0-50 | `mytool run show <run_id> [--node <id>]`：打印节点请求/响应/断言 diff/完整轮询轨迹 | `crates/cli` | P0-40 | 超时用例可完整回放轨迹 | TODO |

### 0.8 Spike 与 M0 评审

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P0-51 | `mock-sut` crate：axum 模拟被测系统（下单、支付、异步回调延迟可配、带 seed 的抖动、可注入故障：拒绝连接/500/改字段） | `crates/mock-sut` | P0-01 | 可独立启动；集成测试可内嵌启动 | TODO |
| P0-52 | 三条样例链路：纯串行、并行分支 c∥d、异步轮询（等支付回调）；含环境文件 | `examples/` | P0-51、P0-20 | 三条均通过 `dag validate` | TODO |
| P0-53 | 确定性 golden 测试：同一 DAG 两次 `--no-llm` 运行，确定性报告逐字节一致；断言 cli 依赖树中无 LLM crate | `crates/cli/tests` | P0-49、P0-52 | 测试通过并纳入检查脚本 | TODO |
| P0-54 | 轮询稳定性脚本：异步链路连续 10 次执行并统计通过率与抖动来源 | `scripts/` | P0-52 | 输出统计表 | TODO |
| P0-55 | 分类器覆盖率评估：用 mock-sut 故障注入收集样本，统计 `unknown` 占比 | `scripts/`、`docs/reviews/` | P0-44、P0-51 | 报告 unknown 占比与待补规则 | TODO |
| P0-56 | M0 联合评审报告（§7.3 三个问题），结论与后续调整写入 `docs/reviews/m0.md` | `docs/reviews/` | P0-54、P0-55、P0-20、P1-13 | 三个问题均有数据结论 | TODO |
| P0-57 | **阶段 0 验收**：逐条核对阶段验收标准并记录证据 | `docs/reviews/phase0.md` | P0-53、P0-50、P0-38、P0-39 | 验收通过 | TODO |

---

## 阶段 1：桌面界面与可视化（§5，4–5 周）

阶段验收：排查一次轮询超时失败，点失败节点右面板即显示完整轨迹和断言 diff。
原则：组件只吃纯数据 props，与渲染目标无关；先 Web（Vite + React），再迁 nsh 外壳。

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P1-01 | 初始化 `web/`：Vite + React + TS（strict）+ ESLint + Prettier + Vitest；检查脚本纳入 `web` lint/test/build | `web/` | P0-02 | `npm run build` 与 `npm test` 通过 | TODO |
| P1-02 | Rust→TS 类型生成（ts-rs 或从 JSON Schema 生成）到 `web/src/types/generated`；检查脚本校验同步 | `crates/model`、`web/` | P1-01、P0-12 | 类型不同步时检查失败 | TODO |
| P1-03 | `server` crate（axum）只读 API：DAG 列表/版本/详情、运行列表、运行详情、节点执行证据 | `crates/server` | P0-40 | API 集成测试 | TODO |
| P1-04 | server SSE：转发 `EngineEvent`，支持按 run 订阅、断线重连补发 | `crates/server` | P1-03、P0-41 | SSE 集成测试 | TODO |
| P1-05 | server 写 API：保存 DAG（pending）、校验、激活、触发运行、取消运行 | `crates/server` | P1-03、P0-47 | API 集成测试；保存前强制闸门 | TODO |
| P1-06 | 前端 `DataSource` 接口 + fixture 实现（读取 `examples/` 导出的运行快照）+ HTTP/SSE 实现 | `web/src/data` | P1-01 | 两种实现通过同一套契约测试 | TODO |
| P1-07 | 应用外壳布局：顶栏/左导航/中央/右面板/底部抽屉/状态栏；§5.1 默认尺寸，可折叠、可拖拽范围限制 | `web/src/shell` | P1-01 | 组件测试覆盖尺寸边界 | TODO |
| P1-08 | Dagre 布局纯函数：DAG → 节点坐标；同输入同输出 | `web/src/canvas/layout` | P1-01 | 单测（确定性） | TODO |
| P1-09 | 画布编辑视图渲染（@xyflow/react）：节点卡片（类型图标+名称+摘要）、带箭头边与变量标注、缩放/平移/小地图/适应画布 | `web/src/canvas` | P1-08、P1-06 | 渲染三条样例链路 | TODO |
| P1-10 | 执行态视图：状态色（灰/蓝脉动/绿/红/黄脉动/深灰）、并行同时脉动、轮询节点「第 n/N 次 · 状态=X」、进度条；沿用静态坐标 | `web/src/canvas` | P1-09 | 用 fixture 事件流回放验证 | TODO |
| P1-11 | 右面板-执行详情：实际请求/响应、断言逐条结果带 diff、耗时分解 | `web/src/panels` | P1-07、P1-06 | 组件测试 | TODO |
| P1-12 | 轮询轨迹：右面板时间线 + 底部抽屉表格（节点/第几次/时间戳/实际状态/距期望值） | `web/src/panels` | P1-11 | 超时 fixture 显示完整轨迹 | TODO |
| P1-13 | 失败即聚焦：失败时画布居中失败节点、右面板切详情、底部日志过滤到该节点 | `web/src` | P1-10、P1-12 | 组件/E2E 测试 | TODO |
| P1-14 | 底部抽屉：实时日志（按节点分组、可过滤）、变量监视 | `web/src/drawer` | P1-07、P1-06 | 组件测试 | TODO |
| P1-15 | 状态栏：存储连接、执行摘要（n/N 节点 · 并行 · 轮询中）、SSE 状态、环境、耗时 | `web/src/shell` | P1-04、P1-07 | 组件测试 | TODO |
| P1-16 | 左导航：场景库（最近状态点 + 通过率迷你条）、计划库（校验徽标）、报告列表 | `web/src/nav` | P1-03、P1-07 | 组件测试 | TODO |
| P1-17 | 中央标签页：多场景/报告标签，双击导航项打开 | `web/src/shell` | P1-16 | 组件测试 | TODO |
| P1-18 | 顶栏运行控制：运行/停止/重跑失败、运行模式切换、环境切换器（含连通性状态点） | `web/src/shell` | P1-05 | E2E 用例 | TODO |
| P1-19 | 引擎：从指定节点起跑，复用已存档上游输出，仅重跑该节点及下游；CLI `run --from-run <id> --from-node <id>` | `crates/engine`、`crates/cli` | P0-40 | 单测：上游节点不再发请求 | TODO |
| P1-20 | UI 节点级重跑：右键「从此节点重跑」、回放视图「重跑此节点及下游」 | `web/src/canvas` | P1-19、P1-05 | E2E 用例 | TODO |
| P1-21 | 编辑视图交互：增删节点、连线、右键菜单、自动布局按钮 | `web/src/canvas` | P1-09 | 组件测试 | TODO |
| P1-22 | 右面板属性表单：http/poll/assert 三类节点编辑，保存为 pending 新版本 | `web/src/panels` | P1-21、P1-05 | E2E：编辑后保存生成新版本 | TODO |
| P1-23 | Body/路径模板变量高亮与补全，列出上游可用变量 | `web/src/panels` | P1-22 | 只补全祖先节点变量 | TODO |
| P1-24 | 实时校验：编辑时调用校验 API，徽标 ✓/⚠/✗，问题列表点击定位节点 | `web/src` | P1-22 | E2E 用例 | TODO |
| P1-25 | 报告回放视图：最终态叠加、摘要条（通过率/耗时/与上次对比/触发方式）、时间轴滑块回放 | `web/src/canvas` | P1-10、P1-03 | 组件测试 | TODO |
| P1-26 | 布局持久化：迁移新增 `ui_layouts`、`workspace_presets` 表；按场景记住缩放/位置/面板宽度/抽屉状态 | `crates/store`、`crates/server`、`web/` | P1-07、P1-03 | 重开后布局恢复 | TODO |
| P1-27 | 快捷键（§5.3 第 5 条）与全局搜索 Ctrl/Cmd+K、`?` 快捷键面板 | `web/src` | P1-17 | 组件测试 | TODO |
| P1-28 | 工作区预设：编排者/执行调试者/看报告者，一键切换 | `web/src` | P1-26 | 组件测试 | TODO |
| P1-29 | Playwright E2E：轮询超时失败从「变红」到右面板看到完整证据 ≤2 次点击 | `web/e2e` | P1-13、P1-04 | E2E 通过并纳入 CI | TODO |
| P1-30 | nsh 技术 spike：验证加载 web 组件、`native_module` 调用 engine/store、`nsh test` 无界面测试；结论写 ADR | `app/`、`docs/decisions/` | P1-09 | ADR 给出可行/不可行与替代方案 | TODO |
| P1-31 | nsh 外壳集成：以 `native_module` 实现 `DataSource`，复用全部 web 组件 | `app/` | P1-30 | `nsh test` 跑通核心用例 | TODO |
| P1-32 | **阶段 1 验收** | `docs/reviews/phase1.md` | P1-29、P1-25、P1-24、P1-31 | 验收通过 | TODO |

---

## 阶段 2：AI 生成与校验闸门（§4.3、§4.10、§6.1、§6.2，4–5 周）

阶段验收：含无效变量引用的 AI 候选入库前被拦截并定位；通过候选激活后行为与手工一致。

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P2-01 | `secrets` crate：解析 `keychain://service/account` 与 `${env:VAR}`；写入钥匙串（keyring）；store 层对 `*_ref` 列做格式守卫，拒绝明文 | `crates/secrets`、`crates/store` | P0-21 | 写入明文凭据返回错误；Windows 凭据管理器实测 | TODO |
| P2-02 | `llm-gateway` crate：`LlmProvider` trait、OpenAI 兼容实现、录制/回放 Mock 实现（测试默认 Mock）；API Key 经 secrets 获取 | `crates/llm-gateway` | P2-01 | Mock 回放单测；真实调用仅在显式环境变量下运行 | TODO |
| P2-03 | 迁移新增 `llm_call_logs`；每次调用记录 purpose/tokens/cost/latency/outcome | `crates/store`、`crates/llm-gateway` | P2-02 | 单测 | TODO |
| P2-04 | 脱敏器 Redactor：发送前扫描提示词，疑似密码/密钥/token 替换为 `****`；gateway 强制经过 | `crates/llm-gateway` | P2-02 | 样本集（.env、yml、JWT、AK/SK）全部被替换 | TODO |
| P2-05 | `importer` crate：归一化 `ApiSpec` 模型 + `ApiSpecRepository`（draft/active/deprecated，版本化） | `crates/importer`、`crates/store` | P0-22 | 单测 | TODO |
| P2-06 | OpenAPI 3.x / Swagger 2.0 确定性解析器 | `crates/importer` | P2-05 | Petstore 等样例解析快照测试 | TODO |
| P2-07 | Postman Collection v2.1 解析器 | `crates/importer` | P2-05 | 样例快照测试 | TODO |
| P2-08 | cURL 命令解析器（请求还原 + 启发式推断） | `crates/importer` | P2-05 | ≥10 条 cURL 用例 | TODO |
| P2-09 | HAR 解析器（去重同一端点、推断响应 schema） | `crates/importer` | P2-05 | 样例快照测试 | TODO |
| P2-10 | 增量合并：新旧 ApiSpec diff，策略 合并/覆盖/跳过 | `crates/importer` | P2-05 | 三种策略单测 | TODO |
| P2-11 | Markdown 导入：AI 抽取结构化草稿（带置信度、来源行号）→ 待审队列（status=draft） | `crates/importer` | P2-04、P2-05 | Mock 回放用例；草稿不进入 active | TODO |
| P2-12 | CLI `mytool api import --format <openapi\|postman\|curl\|har\|md> <file> --strategy <...>` | `crates/cli` | P2-06 ~ P2-11 | 各格式端到端用例 | TODO |
| P2-13 | 导入 UI：预览卡片、低置信度黄色高亮、逐条确认/修正、合并策略选择 | `web/`、`crates/server` | P2-12、P1-07 | E2E 用例 | TODO |
| P2-14 | http 节点关联 `api_spec_id`，闸门字段存在规则改为优先使用 api_specs 的 response_schema | `crates/model`、`crates/gate` | P2-05、P0-17 | 单测 | TODO |
| P2-15 | 提示词资产：`prompts/` 目录版本化模板（含 §10.3），prompt_version 参与缓存键 | `prompts/`、`crates/generator` | P2-02 | 模板加载与渲染单测 | TODO |
| P2-16 | `generator` crate 上下文组装：自然语言描述 + 接口 schema + 业务规则 + Skill 契约 + 偏好（后两者先为空接口） | `crates/generator` | P2-15、P2-05 | 组装结果快照测试；经过 Redactor | TODO |
| P2-17 | 生成与解析：调用 LLM，容错提取 JSON，反序列化为 Dag；只接受 DAG JSON，拒绝任何代码输出 | `crates/generator` | P2-16 | Mock 回放：正常/带解释文本/非法 JSON 三例 | TODO |
| P2-18 | 闸门回喂循环：闸门失败时把 Issue 列表回喂模型修正，最多 N 轮；仍失败标记「需人工介入」 | `crates/generator` | P2-17、P0-20 | 无效变量引用候选被拦截并定位 | TODO |
| P2-19 | 确定性自动校准器（§6.6）：可机械修复项（如引用了非相邻祖先补边、补默认 timeout）自动修复并记录每条修复 | `crates/gate` | P0-20 | 每类修复正反例；修复后再次闸门通过 | TODO |
| P2-20 | 待审区：AI 候选以 `status=pending, created_by=ai` 保存；server/CLI 列表与详情 API | `crates/store`、`crates/server`、`crates/cli` | P2-18、P1-05 | AI 候选无法直接 active（测试） | TODO |
| P2-21 | 激活流程：激活前重新执行闸门 + 自动校准，通过才产生 active 新版本 | `crates/store`、`crates/server` | P2-20、P2-19 | 绕过闸门的激活请求被拒绝 | TODO |
| P2-22 | UI：AI 候选半透明预览、校验徽标、激活按钮、与当前 active 版本 diff | `web/` | P2-20、P1-24 | E2E 用例 | TODO |
| P2-23 | UI：校验失败问题列表「交给 AI 修正」 | `web/`、`crates/server` | P2-22、P2-18 | E2E 用例 | TODO |
| P2-24 | AI 副驾 Tab：对话界面；每条输出绑定工件（插入画布/定位节点/采纳断言）；顶部显示当前上下文 | `web/`、`crates/server` | P2-22 | E2E 用例（Mock 回放） | TODO |
| P2-25 | 生成评测 harness：3 条链路描述 × N 次，统计闸门首次接受率与最终接受率（目标 ≥70%） | `scripts/`、`docs/reviews/` | P2-18 | 输出评测报告 | TODO |
| P2-26 | 一致性测试：AI 生成并激活的 DAG 与手工 DAG 在 mock-sut 上确定性报告一致 | `crates/cli/tests` | P2-21、P0-53 | 测试通过 | TODO |
| P2-27 | **阶段 2 验收** | `docs/reviews/phase2.md` | P2-23 ~ P2-26、P2-13 | 验收通过 | TODO |

---

## 阶段 3：Skill 体系与 nsh 插件（§4.4、§4.9、§6.5，3–4 周）

阶段验收：同一条件查询 Skill 更换条件不需改 DAG；Skill 新版本激活列出所有受影响 pin。

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P3-01 | 迁移新增 `skills`、`skill_references` | `crates/store` | P0-21 | schema 快照测试 | TODO |
| P3-02 | Skill 工件包格式：manifest（名称/semver/依赖）、类型化输入输出契约、内部 DAG、自带断言；JSON Schema 导出 | `crates/skills` | P0-05 | round-trip 与 schema 同步测试 | TODO |
| P3-03 | `SkillRepository` + CLI `mytool skill save/activate/list`；保存前对内部 DAG 跑闸门 | `crates/skills`、`crates/cli` | P3-01、P3-02 | 单测 + CLI 用例 | TODO |
| P3-04 | subworkflow 引用 Skill：`skill_id` + semver 约束解析到具体版本 | `crates/engine` | P3-03、P0-37 | 多版本共存时解析正确 | TODO |
| P3-05 | 闸门规则：Skill 输入绑定与契约类型匹配、必填输入齐全、输出引用存在于契约 | `crates/gate` | P3-04 | 正反例 | TODO |
| P3-06 | DAG 激活时维护 `skill_references` | `crates/store` | P3-04 | 单测 | TODO |
| P3-07 | 影响分析：Skill 新版本激活时列出所有受影响 pin（DAG/版本/节点） | `crates/skills` | P3-06 | 阶段验收用例 | TODO |
| P3-08 | 批量更新 pin：为受影响 DAG 生成 pending 新版本，支持逐个确认 | `crates/skills`、`crates/server`、`web/` | P3-07 | E2E 用例 | TODO |
| P3-09 | 迁移新增 `db_connections`；`ConnectionRepository`；环境 → 连接绑定 | `crates/store` | P2-01 | 密码字段只允许引用（测试） | TODO |
| P3-10 | db_query 执行器（sqlx：Postgres/MySQL/SQLite）：参数化查询、结果映射；`read_only` 连接强制只读事务并拒绝写语句 | `crates/engine` | P3-09、P0-09 | 只读连接执行 UPDATE 失败；testcontainers 或 SQLite 用例 | TODO |
| P3-11 | 认证方案：环境级 API Key/Bearer/Basic/OAuth2 client credentials；`${auth.x}` 运行时注入，值不入日志与快照（脱敏） | `crates/engine`、`crates/secrets` | P2-01、P0-32 | 快照中凭据为 `****` | TODO |
| P3-12 | 认证 Skill + token 复用（同运行/同环境内缓存，过期刷新） | `skills/`、`crates/engine` | P3-11、P3-04 | 多次调用只登录一次 | TODO |
| P3-13 | 条件查询 Skill：结构化查询意图 → Skill 模板匹配 → 只读查询，产出实体 ID | `skills/`、`crates/skills` | P3-10、P3-04 | 换条件不改 DAG（阶段验收用例） | TODO |
| P3-14 | 自然语言条件 → 查询意图（LLM 兜底）+ L1 缓存（键：条件文本 + schema 哈希 + 提示词版本） | `crates/generator` | P3-13、P2-15 | 缓存命中不调 LLM（测试） | TODO |
| P3-15 | L2 查询结果缓存：按 run_id 隔离、短 TTL | `crates/engine` | P3-13 | 单测 | TODO |
| P3-16 | Skill 库 / 子计划库 UI：列表、引用次数、拖拽到画布生成 subworkflow 节点 | `web/` | P3-03、P1-21 | E2E 用例 | TODO |
| P3-17 | 插件包格式与签名：manifest + TSX 产物 + ed25519 签名；宿主校验签名 | `crates/skills`、`web/` | P3-02 | 篡改包被拒绝 | TODO |
| P3-18 | 插件沙箱加载：Web 目标用 sandbox iframe + postMessage 类型化 RPC；nsh 目标走原生沙箱（依赖 P1-30 结论） | `web/src/plugins`、`app/` | P3-17 | 插件无法访问宿主 DOM/存储 | TODO |
| P3-19 | 插件 API：自定义节点外观与详情面板；示例插件「轮询进度 第 3/15 次 · 状态=PENDING」 | `web/src/plugins`、`plugins/` | P3-18 | 示例插件端到端可用 | TODO |
| P3-20 | 相似子链路检测：对多个 DAG 计算子图指纹，重复 ≥ 阈值时提出抽取建议 | `crates/skills` | P3-02 | 构造重复子链路用例 | TODO |
| P3-21 | 一键抽取：将选中子图抽为 Skill 并替换原位置为 subworkflow 节点（生成 pending 版本） | `crates/skills`、`web/` | P3-20、P3-16 | 抽取前后确定性报告一致 | TODO |
| P3-22 | **阶段 3 验收** | `docs/reviews/phase3.md` | P3-07、P3-13、P3-19、P3-21 | 验收通过 | TODO |

---

## 阶段 4：六缺口收束与 CI 嵌入（§4.5–§4.12，4–5 周）

阶段验收：上游删字段执行前标橙；`connection_error` 不触发 LLM；失败执行后数据残留为 0。

### 4.1 契约漂移检测（§4.6）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P4-01 | 迁移新增 `api_schema_fingerprints`、`drift_events`、`drift_impacts` | `crates/store` | P0-21 | schema 快照测试 | TODO |
| P4-02 | `drift` crate：响应形状指纹（字段路径集合 + 类型 + 嵌套深度）规范化哈希 | `crates/drift` | P0-11 | 字段顺序无关；类型变化哈希变化 | TODO |
| P4-03 | 指纹 diff 与严重度分级（删字段/改类型=high，新增字段=low） | `crates/drift` | P4-02 | 单测 | TODO |
| P4-04 | 只读探测任务：仅对标记可探测的接口（默认 GET）调用，可配置频率（默认每日）；CLI `mytool drift scan` | `crates/drift`、`crates/cli` | P4-03、P4-01 | mock-sut 改字段后被检测 | TODO |
| P4-05 | 影响分析：反查引用该接口的 DAG 节点/Skill 及引用路径，写 `drift_impacts`（broken_ref/type_mismatch/assert_invalid） | `crates/drift` | P4-04、P3-06 | 删字段用例定位到具体节点与 ref_path | TODO |
| P4-06 | 迁移候选：生成修正后的 DAG 进入待审区，不自动修改 active | `crates/drift`、`crates/generator` | P4-05、P2-20 | active 版本不变（测试） | TODO |
| P4-07 | UI：受影响节点标橙、漂移事件列表与处理状态 | `web/` | P4-05 | E2E：执行前标橙 | TODO |

### 4.2 自主性策略与熔断（§4.5、§4.11）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P4-08 | 迁移新增 `autonomy_policy`（含 §4.5 默认种子）、`llm_budgets`、`repair_success_rates` | `crates/store` | P0-21 | 默认策略种子测试 | TODO |
| P4-09 | `autonomy` crate：category → 动作（retry / llm_repair / manual）决策；engine 只暴露失败钩子，**不依赖 autonomy** | `crates/autonomy` | P4-08、P0-44 | 依赖守卫通过 | TODO |
| P4-10 | connection_error/timeout：自动重试一次（新 attempt），仍失败标记环境问题；全程不调 LLM | `crates/autonomy`、`crates/engine` | P4-09 | 测试断言 `llm_call_logs` 为空 | TODO |
| P4-11 | assertion_failed_*：LLM 生成候选修复 → 子进程中 `mytool run --isolated` 真实执行通过 → 才提交人工审 diff | `crates/autonomy`、`crates/cli` | P4-10、P2-17 | 执行未通过的修复不会出现在待审区 | TODO |
| P4-12 | 修复 diff 审阅 UI（采纳/拒绝，采纳生成 pending 版本） | `web/` | P4-11、P2-22 | E2E 用例 | TODO |
| P4-13 | 三级预算（全局/Skill/场景）调用前检查：次数与成本上限 | `crates/llm-gateway` | P4-08、P2-03 | 超预算拒绝调用并计熔断 | TODO |
| P4-14 | 成功率熔断：分类历史修复成功率 < 10% 跳过 LLM；调用后回写成功率 | `crates/llm-gateway`、`crates/autonomy` | P4-13 | 单测 | TODO |
| P4-15 | 报告固定展示「AI 调用 X 次 · 预估成本 Y · 熔断 Z 次」（CLI + UI） | `crates/cli`、`web/` | P4-14 | 快照测试 | TODO |

### 4.3 修正知识库（§4.7）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P4-16 | 迁移新增 `correction_logs`、`preference_suggestions` | `crates/store` | P0-21 | schema 快照测试 | TODO |
| P4-17 | 修正记录：用户修改 AI 生成物（采纳前/激活后）时计算 JSON diff，归类 correction_type，生成 pattern_key | `crates/generator`、`crates/server` | P4-16、P2-22 | 单测 | TODO |
| P4-18 | 聚合：同一 pattern_key ≥3 次生成偏好建议；接受/拒绝（dismissed 不再提示） | `crates/generator`、`web/` | P4-17 | 第 3 次出现时产生建议 | TODO |
| P4-19 | 偏好注入：已接受偏好进入生成上下文；验证后续生成不再犯同类错误 | `crates/generator` | P4-18、P2-16 | Mock 回放用例 | TODO |

### 4.4 数据生命周期（§4.8）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P4-20 | 迁移新增 `test_data_records`；节点可声明「产出实体」，执行时同步登记 | `crates/store`、`crates/engine` | P0-40 | 单测 | TODO |
| P4-21 | cleanup 按 run_id 回收：delete/restore/mark 三种策略，记录状态与错误 | `crates/engine` | P4-20、P0-36 | 失败运行后残留为 0（mock-sut 用例） | TODO |
| P4-22 | 报告汇总「使用 X / 回收 X / 残留 X」；下次运行开始提醒残留，提供手动清理与重试（CLI + UI） | `crates/cli`、`web/` | P4-21 | E2E 用例 | TODO |

### 4.5 连接发现（§4.9）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P4-23 | 项目目录只读扫描：限定 glob（`.env*`、`application*.yml`、`docker-compose*.yml`、`config/`、`*.toml`），疑似凭据占位替换 | `crates/importer` | P2-04 | 扫描结果中无明文凭据（测试） | TODO |
| P4-24 | AI 生成连接候选草稿（置信度 + 来源文件）→ 用户确认 → 从本地读取真实值写入钥匙串 | `crates/importer`、`web/` | P4-23、P3-09 | 模型输入中无明文（Mock 断言） | TODO |

### 4.6 CI/CD 嵌入（§4.12、§8.2）

| ID | 任务 | 位置 | 依赖 | 验收标准 | 状态 |
|---|---|---|---|---|---|
| P4-25 | 场景 tag 模型与 `--scenario <id\|tag>` 选择 | `crates/store`、`crates/cli` | P0-48 | 单测 | TODO |
| P4-26 | 迁移新增 `gate_policies`；`--gate` 判定：P0 场景通过率 <100% 退出码 1，`block_on_drift` 且有未处理漂移退出码 2 | `crates/cli` | P4-25、P4-04 | 两种退出码用例 | TODO |
| P4-27 | JUnit XML 报告 `--report junit.xml` | `crates/cli` | P0-49 | XSD 校验通过 | TODO |
| P4-28 | CI 运行写回 `dag_runs(mode=ci)`，桌面端可查看同一次运行 | `crates/cli`、`web/` | P4-27、P1-25 | 用例 | TODO |
| P4-29 | 验收看板指标：确定性一致率、timeout 的 LLM 调用数、漂移前置发现数、修正复用率、数据残留数；CLI `mytool metrics` | `crates/cli` | P4-10、P4-05、P4-18、P4-22 | 指标计算单测 | TODO |
| P4-30 | CI 模板示例（GitHub Actions / GitLab CI）与使用文档 | `examples/ci/`、`docs/` | P4-26、P4-27 | 模板在本仓库 CI 中实跑 | TODO |
| P4-31 | 五个核心验收问题（§8.1）端到端测试套件 | `tests/acceptance/` | P4-07、P4-11、P4-15、P4-19、P4-22 | 五条全部通过 | TODO |
| P4-32 | **阶段 4 验收** | `docs/reviews/phase4.md` | P4-31、P4-29、P4-30 | 验收通过 | TODO |

---

## 待决问题

需要人类决策的问题登记在此。任务因此阻塞时状态改为 `BLOCKED` 并引用问题编号。

| # | 问题 | 影响任务 | 默认假设（未决前按此推进） | 状态 |
|---|---|---|---|---|
| Q1 | nsh 框架的获取方式、版本与 API 文档在哪里？ | P1-30、P1-31、P3-18 | 阶段 0–1 只做 Web 目标；nsh 相关任务等 spike 结论 | OPEN |
| Q2 | CLI 二进制正式名称（文档中为 `mytool`） | P0-45 | 暂用 `mytool` | OPEN |
| Q3 | 默认 LLM 提供方与模型 | P2-02 | OpenAI 兼容接口，base_url/model 可配置 | OPEN |
| Q4 | db_query 首批需支持的数据库 | P3-10 | Postgres、MySQL、SQLite | OPEN |
| Q5 | JSONPath 方言 | P0-34、P0-07 | 暂按 RFC 9535 的 singular query 子集（`$`、`.name`、`['name']`、`[n]`）自行解析（`model::parse_json_path`，与模板路径共用解析器），不引入 serde_json_path；若需要通配/过滤再议 | OPEN |
| Q6 | 用于 spike 的 3 条「真实链路」由谁提供？ | P0-52 | 先用 mock-sut 构造等价链路 | OPEN |
| Q7 | 代码托管远程仓库（GitHub / Cursor origin / 其他）？ | P0-03 | GitHub：`git@github.com:deliangyang/ai-testing.git`（2026-11-08 人类答复） | RESOLVED |

---

## 变更记录

按时间追加，格式：`YYYY-MM-DD | 任务 ID | 状态变化 | 说明（提交哈希/偏差/新增任务）`

| 日期 | 任务 | 变化 | 说明 |
|---|---|---|---|
| 2026-11-08 | — | 创建 | 根据 develop.md v1.0 拆分初版任务表 |
| 2026-11-08 | P0-01 | TODO→DONE | 本地工具链为 Rust stable（x86_64-pc-windows-gnu）；rust-toolchain.toml 不固定 target，以便 CI 在 Linux 上使用默认 target |
| 2026-11-08 | P0-02 | TODO→DONE | 新增 `.gitattributes` 强制 LF（逐字节一致测试依赖统一换行）；sleep 白名单为 `crates/common/src/clock.rs` 与 `crates/mock-sut/`；守卫已用人为注入的违规验证 |
| 2026-11-08 | P0-03 | TODO→BLOCKED | 已提交 `.github/workflows/ci.yml`（ubuntu + windows 矩阵）；无远程仓库无法实跑，见 Q7 |
| 2026-11-08 | P0-04 | TODO→DONE | `Timestamp` 为毫秒精度 UTC，序列化为 RFC 3339；`ManualSleeper` 采用虚拟时间（立即返回并推进配对的 `FixedClock`）；ID 生产实现用 UUID v7（按时间有序）。本地 GNU 工具链需要 MinGW（WinLibs gcc 16.2）提供 dlltool |
| 2026-11-08 | P0-05 | TODO→DONE | 节点为扁平对象 `{id, name?, type, ...配置}`；经中间结构反序列化，使各类型配置仍可 `deny_unknown_fields`；YAML 库选用 `yaml_serde`（YAML 组织维护的 serde_yaml 后继）；六类节点配置暂为最小字段，由 P0-06~P0-09 补全 |
| 2026-11-08 | P0-06 | TODO→DONE | 新增可复用的 `HttpRequest`（供 poll 使用）；`response_schema` 描述的是响应 body；method 接受大写或小写；新增 `timeout_ms` 可选字段 |
| 2026-11-08 | P0-07 | TODO→DONE | 字段名为 `request`/`condition`/`backoff_ms`/`timeout_ms`，另有可选 `max_attempts`、`response_schema`；`Condition{path, op, expected?}` 与 `Operator` 枚举供 assert 复用；路径为作用于 `{status, headers, body}` 的 JSONPath；backoff 为空数组的情况留给闸门规则 P0-18 处理 |
| 2026-11-08 | P0-08 | TODO→DONE | 与任务描述的偏差：一个 assert 节点为 `target` + `assertions: [Condition]` 列表（而非单条 path/operator/expected），以支持逐条结果；运算符字段名为 `op`，与 poll 条件一致 |
| 2026-11-08 | P0-09 | TODO→DONE | db_query 使用位置参数 `params` 和 `result: rows/first_row/scalar`；子工作流引用为 `dag: {id, version?}`（不填版本表示运行时取 active），子 DAG 通过保留节点 id `input` 读取输入（`${input.output.x}`），父 DAG 只能看到 `outputs` 映射；cleanup 在阶段 0 为按顺序执行的 HTTP 请求列表。保留 id `input`/`env`/`auth` 需由闸门规则禁止用作节点 id（并入 P0-14） |
| 2026-11-08 | P0-10 | TODO→DONE | 路径段支持 `.key`、`[n]`、`["key"]`/`['key']`；非 ASCII 字段名必须用引号形式；`refs_in_value` 按文档顺序返回引用及其 JSON Pointer；`as_single_ref` 供渲染器在整串只有一个引用时保留原 JSON 类型 |
| 2026-11-08 | P0-11 | TODO→DONE | 浮点数按 RFC 8785/ECMAScript 规则输出（基于标准库最短往返数字），不依赖 serde_json 内部格式化器，避免升级依赖导致哈希变化；workspace 开启 serde_json `float_roundtrip`，否则解析会差 1 ULP（属性测试发现）；键按 UTF-8 字节序排序（JCS 为 UTF-16，ASCII 键结果相同） |
| 2026-11-08 | P0-12 | TODO→DONE | schemars 1.x，draft 2020-12；`Node` 的 schema 手写为 `oneOf`，每种节点一个封闭对象；更新命令 `UPDATE_SCHEMAS=1 cargo test -p model --test schema_sync`；额外用 jsonschema 校验样例文档，并验证 7 类错误 schema 与解析器同样拒绝 |
| 2026-11-08 | P0-13 | TODO→DONE | 问题码为点分小写字符串（如 `structure.duplicate_node_id`），位置用 JSON Pointer；排序键为 (node_id, code, json_path, severity, message)，并去除完全重复的问题；DAG 级问题（无 node_id）排在最前；warn 不阻塞 `passed()` |
| 2026-11-08 | P0-14 | TODO→DONE | 额外增加 `structure.invalid_node_id`：节点 id 必须匹配 `[A-Za-z_][A-Za-z0-9_-]*`，与模板引用语法一致；新增 `Gate::standard()` 注册全部内置规则；重复 id 定位到后出现的那一处 |
| 2026-11-08 | P0-15 | TODO→DONE | 拓扑排序与找环放在 `model::graph::DagGraph`（引擎 P0-28 复用，引擎不依赖 gate）；就绪节点按 id 字典序出队；环路径旋转为从最小 id 开始，并定位到环上第一条边；自环与悬空边在图中忽略，由结构规则报告 |
| 2026-11-08 | P0-16 | TODO→DONE | 模板位置提取放在 `model::scope::node_refs`（引擎渲染复用）：覆盖 http/poll 请求各部分、条件与断言的 expected、db_query params、subworkflow inputs、cleanup 请求；subworkflow outputs 属子作用域，留给 P0-37；assert `target` 同样要求为祖先；`${input.output...}` 不算节点引用；模板语法错误报 `reference.invalid_template` |
| 2026-11-08 | P0-17 | TODO→DONE | 约定节点输出结构：http/poll 为封闭对象 {status, headers, body, timing}，body 由 response_schema 描述；db_query 为 rows/row/value（schema 描述单行）；subworkflow 为 outputs 的键；assert 为 {results}；cleanup 无输出。schema 遍历（`gate::schema_path`）支持 properties/additionalProperties/items/prefixItems/本地 $ref/allOf/anyOf/oneOf。无 schema 时报 `reference.unverified_field`（warn）；只检查祖先目标，非法目标交给 ReferenceRule。poll/assert 条件里 JSONPath 的字段检查延后 |
| 2026-11-08 | P0-18 | TODO→DONE | 新增 `DecidabilityRule`（规则名 decidability），同时作用于 poll 的 condition 和 assert 的每条 assertion：路径必须是 singular JSONPath，且在已知 schema 中存在（poll 查自身输出，assert 查祖先 target 的输出）；需要 expected 的操作符缺 expected、exists/not_exists 带 expected、gt/gte/lt/lte 非数字、len_eq 非非负整数、regex 无法编译（Rust regex 方言）、type_is 不在 7 种 JSON 类型内，均报错；含模板引用的 expected 留到运行时判断。poll 另查：backoff_ms 非空、单调非减（定位到第一个下降处）、timeout_ms ≥ backoff 首项、max_attempts ≠ 0。顺带修复 model：显式 `expected: null` 原先被反序列化为缺失，现保留为 null。输出结构辅助函数移到 `gate::output` 与 FieldRule 共用 |
| 2026-11-08 | P0-19 | TODO→DONE | 新增 `CompletenessRule`（规则名 completeness）。必填字段（serde 已保证字段存在，这里查空白/空集合）：DAG id、nodes，http path、poll request.path、assert target 与 assertions、db_query connection 与 sql、subworkflow dag.id、cleanup requests 及每个 request 的 path。孤立节点：节点数 ≥ 2 时，没有任何有效边（悬空边、自环不计）的节点报错；cleanup 不豁免。cleanup 的每条出边报错，定位到 /edges/i。http 节点既不是任何 assert 的 target、也没被 assert 中的模板引用时报 warn。测试辅助函数 `validate` 改为过滤掉 completeness.*，供只关注单条规则的测试使用；完整报告用 `validate_all` |
| 2026-11-08 | P0-20 | TODO→DONE | 语料位于 `crates/gate/tests/fixtures/bad_dags/`，共 27 个：结构 6、环 2、引用 5（含 assert target 非祖先）、字段 1、轮询 5、断言 2、完整性 3、加载失败 3（缺 timeout_ms、未知节点类型、配置键拼错）。文件头 `# expect: code[, code]`，`parse` 表示必须在反序列化阶段失败。判定要求 error code 集合与期望完全相等（warn 不计），防止样例靠别的错误被拦截；已用改错期望的方式验证测试会失败。另加 `good_dags/order_pay.yaml`，用覆盖全部节点类型的完整流程防止误杀 |
| 2026-11-08 | P0-21 | TODO→DONE | 选用 rusqlite 0.41（bundled，自带 SQLite 源码，无系统依赖）；它要求 Rust 1.89，因此 workspace 的 rust-version 从 1.80 提到 1.89。`Db::open` 依次设置 busy_timeout=5s、WAL、foreign_keys=ON、synchronous=NORMAL，然后迁移。切换 WAL 时 SQLite 不走 busy handler，并发首次打开会立刻报 locked；改为先读当前模式，遇到 BUSY 时用 `BEGIN IMMEDIATE` 借 busy handler 等待后重试（最多 20 次），不引入裸 sleep，8 线程并发打开连跑 30 次通过。迁移：`migrations/V###__name.sql` 用 include_str! 编进 `MIGRATIONS`，单元测试保证目录里的文件与注册表一致；`schema_migrations` 记录 version/name/sha256/applied_at（applied_at 取注入的 Clock）；整次迁移在一个 IMMEDIATE 事务里，失败则全部回滚；已应用的迁移被改动、库里有本版本不认识的迁移、在已应用版本之下插入新迁移，都会报错 |
| 2026-11-08 | P0-22 | TODO→DONE | `V001__core.sql` 按 §3.1 建 5 张表，在原设计上加了约束：全部 STRICT；必填列 NOT NULL；状态/模式等枚举列加 CHECK（节点状态含 POLLING、SKIPPED，与 §3.2 一致）；`*_json` 列加 json_valid；dag_runs (dag_id, dag_version) 外键指向 dag_definitions；node_executions、failure_records 外键指向 dag_runs 并级联删除；部分唯一索引 `dag_definitions_one_active` 保证同一 DAG 至多一个 active；按查询场景加了 dag_runs、failure_records 的索引。failure_category 暂不加 CHECK，留给 classifier 定义分类。快照文件为 `crates/store/tests/snapshots/schema.sql`，更新命令：设置 UPDATE_SCHEMAS=1 后运行 `cargo test -p store --test schema_snapshot` |
| 2026-11-08 | P0-23 | TODO→DONE | `DagRepository::new(&Connection, &dyn Clock)`，提供 save/activate/get/active/versions。save 在 IMMEDIATE 事务里取 max(version)+1，存去掉 version 后的规范化 JSON，状态为 pending，读回时把 version 填进 Dag。activate 采用乐观并发：调用方传入它看到的当前 active 版本（`expected_active`），与库中不一致返回 `StoreError::Conflict`，且不做任何修改；重复激活当前 active 版本不做操作；允许重新激活 archived 版本（即回滚）；版本不存在返回 NotFound。另有部分唯一索引兜底，保证不会出现两个 active。store 层不跑闸门，保存前由调用方负责过闸门。新增错误类型 NotFound/Conflict/Corrupt。8 线程并发激活恰好 1 个成功、并发保存版本号互不重复，各连跑 20 次通过 |
| 2026-11-08 | P0-24 | TODO→DONE | `RunRepository::new(&Connection, &dyn Clock, &dyn IdGen)`，提供 create/transition/get/list。合法迁移：PENDING→RUNNING/FAILED/CANCELLED，RUNNING→COMPLETED/FAILED/CANCELLED；终态不能再迁移。允许 PENDING→FAILED，用于启动前就失败的情况，比如环境解析失败。进入 RUNNING 时写 started_at，进入终态时写 finished_at，时间都取注入的 Clock。UPDATE 带上原状态作条件，被并发改掉时返回 Conflict。非法迁移返回 `StoreError::InvalidTransition`；创建时 DAG 版本不存在返回 NotFound。list 支持按 dag_id/status/limit 过滤，按 id 倒序，id 为 UUIDv7，按时间有序，因此不用加 created_at 列。矩阵测试覆盖全部 25 种 (from, to) 组合，期望结果单独列出，不依赖被测函数推导 |
| 2026-11-08 | P0-03 | BLOCKED→DOING | Q7 已答复：远程为 GitHub `deliangyang/ai-testing`。远程上原有一个只含 MIT LICENSE 的初始提交，用 `--allow-unrelated-histories` 合并进来，没有改写本地历史，也没有强推；随后把 Cargo 的 license 从 UNLICENSED 改为 MIT，与仓库一致 |
| 2026-11-08 | P0-02-fix1 | 新增→DONE | 首次 Linux CI 报 fmt 失败（P0-24 提交的 runs.rs 未格式化），本地 check.ps1 却显示全绿。原因：`Invoke-Native` 里 `& $Command` 的 stdout 进入函数返回值，变成`输出行 + 布尔值`组成的数组，非空数组恒为真，所以写 stdout 的 fmt/test 步骤失败从不被判定；clippy 写 stderr，因此之前能抓到。改为 `| Out-Host`，并用 stash 回 HEAD 版 runs.rs 的方式验证：未格式化时退出码 1、格式化后为 0。用修好的脚本重跑完整 check 也通过，说明此前没有被掩盖的测试失败，只有这一处格式问题。一并提交 runs.rs 的格式化。Windows CI 首轮的通过同样不可信，以修复后的结果为准 |
| 2026-11-08 | P0-25 | TODO→DONE | `NodeExecutionRepository::new(&Connection, &dyn Clock)`，提供 insert/update/get/latest/list_for_run。新的尝试必须 attempt ≥ 1 且为非终态，(run, node, attempt) 已存在返回 Conflict。状态机按 §3.2：PENDING→READY/SKIPPED，READY→RUNNING/SKIPPED，RUNNING→POLLING/COMPLETED/FAILED，POLLING→COMPLETED/FAILED；允许非终态原地更新（轮询期间更新 poll_trace）。终态行再修改返回 `StoreError::TerminalRow`；FAILED 必须带 failure_category 和 error_code，非 FAILED 不能带。进入 RUNNING 时写 started_at，进入终态时写 finished_at。数据库层另加 V002 迁移：触发器禁止修改终态行、禁止写入未分类的 FAILED 行，绕过仓储直接写 SQL 也会被拦下。list_for_run 按 (node_id, attempt) 排序，不受并行完成顺序影响。input_resolved 中凭据的打码由调用方（引擎）负责 |
