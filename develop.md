# AI 测试系统开发文档

**版本**：v1.0  
**状态**：设计定稿，可进入开发  
**目标读者**：后端/引擎开发、桌面端开发、测试架构师

---

## 1. 概述

### 1.1 项目背景

现有接口测试依赖人工编写脚本或手工搭建场景，存在三个结构性问题：编排不可复用（每条链路从零写）、异步场景易 flaky（裸 `sleep` 导致时通时不通）、失败定位靠翻日志（脚本堆砌无执行态视图）。AI 的介入能显著降低编排成本，但直接让模型生成可执行代码会导致“语法正确、一跑就挂”或“断言写反、静默通过”。

本系统要解决的核心矛盾是：**如何让 AI 承担“场景推演和用例草稿”的高效工作，同时让确定性执行层和人工判断力守住“业务断言正确性”和“数据合法性”这两条底线。**

### 1.2 系统定位

一套**本地优先的 AI 辅助接口测试桌面程序**：声明式 DAG 编排 + 确定性执行引擎 + AI 生成候选（经校验闸门）+ 可视化执行态与报告 + 可复用 Skill 资产库 + CI 确定性门禁。

### 1.3 核心设计原则

1. **确定性执行与 AI 生成彻底分离**：已激活 DAG 在 `--no-llm` 模式下逐字节确定，不依赖任何模型调用。AI 只作用于“生成候选”和“分类后决定是否修复”两个入口。
2. **AI 产物永不直接生效**：生成 → 确定性校验闸门 → 待审区 → 人工确认 → 激活。不存在“AI 直接写入可执行路径”的通道。
3. **敏感信息不进模型、不进 SQLite**：凭据只存钥匙串引用，AI 扫描项目目录时对疑似密码做占位替换。
4. **复用来自沉淀，而非预设**：Skill 从重复模式中抽取，修正偏好从人工操作中聚合，连接从项目发现后确认。
5. **失败即分类，分类即决策**：每次失败产出稳定的 `failure_category` 枚举，自主性策略据此决定重试、调 LLM 还是转人工。

---

## 2. 总体架构

### 2.1 分层架构

```
┌─────────────────────────────────────────────────────────────────┐
│  界面层（nsh：Rust 核心 + TSX 画布/面板 + 沙箱插件）              │
│  画布三模式（编辑/执行态/回放）│ 右上下文面板 │ 底部日志/轮询轨迹  │
├─────────────────────────────────────────────────────────────────┤
│  编排层（声明式 DAG 资产，YAML/JSON，版本化存 SQLite）             │
│  节点：http / poll / assert / db_query / subworkflow / cleanup   │
│  边：依赖 + 变量绑定（${node.output.field}）                      │
├─────────────────────────────────────────────────────────────────┤
│  AI 生成层（仅在“生成候选”和“分类后修复”时介入）                  │
│  生成器 → 确定性校验闸门 → 待审区 → 人工确认 → 激活               │
│  LLM 预算/熔断 │ 修正知识库 │ 契约漂移检测                        │
├─────────────────────────────────────────────────────────────────┤
│  执行引擎（确定性，可 CLI，--no-llm 模式无任何模型调用）           │
│  拓扑调度 │ 并行分支 │ 轮询节点（退避+超时+轨迹） │ 子工作流帧栈   │
│  失败分类器 → 自主性策略 → 节点级重跑                             │
├─────────────────────────────────────────────────────────────────┤
│  持久化层（SQLite + 系统钥匙串）                                  │
│  版本化 DAG │ 运行状态机 │ 节点执行快照 │ Skill 索引 │ 证据存档    │
│  敏感凭据只存引用，真实值在钥匙串                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 技术选型

| 层 | 选型 | 理由 |
|---|---|---|
| 桌面框架 | **nsh（Rust + TSX 混合）** | Rust 核心负责引擎/SQLite/钥匙串；TSX 负责画布与面板。支持运行时加载签名 TSX 模块作为沙箱插件，承载 Skill 可视化扩展。`nsh test` 提供无界面测试能力 |
| DAG 自动布局 | **Dagre**（`@dagrejs/dagre` 或 `@foblex/flow-dagre-layout`） | 布局计算与渲染分离。执行态沿用静态坐标，只叠加状态色 |
| 持久化 | **SQLite（rusqlite / SQLx）**，WAL 模式 | 版本化 DAG 定义、节点状态机、追加写入执行历史 |
| 执行引擎 | **自研**（参考 hexdag 的声明式 YAML + DAG 模型） | 需精确控制轮询节点、子工作流帧栈、失败分类钩子 |
| CLI | **Rust 独立二进制**，与 nsh 共用 engine crate | `mytool run --scenario <id\|tag> --env <name> --no-llm --gate --report junit.xml` |
| 凭据存储 | **OS 钥匙串（keyring crate）+ 环境变量** | SQLite 只存 `keychain://...` 或 `${env:...}` 引用 |

### 2.3 模块依赖关系

```
engine ──→ store（SQLite）
  │           │
  ├──→ classifier（失败分类）
  ├──→ poller（轮询退避）
  └──→ subworkflow（帧栈）
        │
cli ────┘
app（nsh）──→ engine / store / llm-gateway / drift-detector
                │
llm-gateway ──→ budget / circuit-breaker / knowledge-base
                │
drift-detector ──→ schema-fingerprint
```

---

## 3. 数据模型

### 3.1 核心表结构

```sql
-- 接口定义（导入后归一化）
CREATE TABLE api_specs (
  id TEXT PRIMARY KEY, name TEXT, method TEXT, path TEXT,
  params_json TEXT, response_schema_json TEXT,
  auth_scheme_ref TEXT, version INTEGER, status TEXT -- draft/active/deprecated
);

-- DAG 定义（版本化）
CREATE TABLE dag_definitions (
  id TEXT, version INTEGER, content_json TEXT, status TEXT, -- pending/active/archived
  created_by TEXT, created_at TEXT, PRIMARY KEY (id, version)
);

-- DAG 运行实例（状态机唯一事实来源）
CREATE TABLE dag_runs (
  id TEXT PRIMARY KEY, dag_id TEXT, dag_version INTEGER,
  env_name TEXT, mode TEXT, -- manual/ci
  status TEXT, -- PENDING/RUNNING/COMPLETED/FAILED/CANCELLED
  started_at TEXT, finished_at TEXT
);

-- 节点执行（追加写入，证据存档）
CREATE TABLE node_executions (
  run_id TEXT, node_id TEXT, status TEXT, -- 含 POLLING
  attempt INTEGER, input_resolved_json TEXT,
  output_json TEXT, poll_trace_json TEXT,
  failure_category TEXT, error_code TEXT,
  started_at TEXT, finished_at TEXT,
  PRIMARY KEY (run_id, node_id, attempt)
);

-- 失败记录
CREATE TABLE failure_records (
  id TEXT PRIMARY KEY, run_id TEXT, node_id TEXT,
  failure_category TEXT, error_code TEXT, raw_message TEXT,
  classified_at TEXT, resolution TEXT
);

-- 自主性策略
CREATE TABLE autonomy_policy (
  failure_category TEXT PRIMARY KEY,
  autonomy_level TEXT, -- manual/gated/supervised/autonomous
  max_retry INTEGER, llm_allowed INTEGER, updated_at TEXT
);

-- 契约漂移
CREATE TABLE api_schema_fingerprints (
  api_id TEXT, schema_hash TEXT, recorded_at TEXT,
  sample_response_json TEXT, PRIMARY KEY (api_id, schema_hash)
);
CREATE TABLE drift_events (
  id TEXT PRIMARY KEY, api_id TEXT, old_hash TEXT, new_hash TEXT,
  diff_json TEXT, detected_at TEXT, severity TEXT, status TEXT
);
CREATE TABLE drift_impacts (
  drift_event_id TEXT, node_id TEXT, ref_path TEXT,
  impact_type TEXT, -- broken_ref/type_mismatch/assert_invalid
  PRIMARY KEY (drift_event_id, node_id, ref_path)
);

-- Skill 库
CREATE TABLE skills (
  id TEXT, version INTEGER, name TEXT, content_json TEXT,
  input_contract_json TEXT, output_contract_json TEXT,
  tsx_plugin_ref TEXT, status TEXT,
  PRIMARY KEY (id, version)
);
CREATE TABLE skill_references (
  skill_id TEXT, skill_version INTEGER, dag_id TEXT, dag_version INTEGER,
  PRIMARY KEY (skill_id, skill_version, dag_id, dag_version)
);

-- 修正知识库
CREATE TABLE correction_logs (
  id TEXT PRIMARY KEY, target_type TEXT, target_id TEXT,
  diff_json TEXT, correction_type TEXT, operator TEXT, created_at TEXT
);
CREATE TABLE preference_suggestions (
  id TEXT PRIMARY KEY, target_id TEXT, pattern_key TEXT,
  occurrence_count INTEGER, suggested_default TEXT,
  status TEXT -- pending/accepted/dismissed
);

-- AI 预算与调用日志
CREATE TABLE llm_budgets (
  scope_type TEXT, scope_id TEXT,
  max_calls_per_run INTEGER, max_cost_per_run REAL,
  current_window_start TEXT,
  PRIMARY KEY (scope_type, scope_id)
);
CREATE TABLE llm_call_logs (
  id TEXT PRIMARY KEY, run_id TEXT, node_id TEXT, purpose TEXT,
  tokens_in INTEGER, tokens_out INTEGER, cost REAL,
  latency_ms INTEGER, outcome TEXT
);
CREATE TABLE repair_success_rates (
  failure_category TEXT PRIMARY KEY,
  attempts INTEGER, successes INTEGER, updated_at TEXT
);

-- 数据生命周期
CREATE TABLE test_data_records (
  id TEXT PRIMARY KEY, run_id TEXT,
  source_type TEXT, -- discovered/created/template
  source_ref TEXT, entity_type TEXT, entity_id TEXT,
  cleanup_strategy TEXT, -- delete/restore/mark
  planned_cleanup_at TEXT, cleanup_status TEXT, cleanup_error TEXT
);

-- CI 门禁策略
CREATE TABLE gate_policies (
  scenario_id TEXT PRIMARY KEY,
  min_pass_rate REAL, block_on_drift INTEGER, block_on_p0_fail INTEGER
);

-- 连接配置
CREATE TABLE db_connections (
  name TEXT PRIMARY KEY, type TEXT, host TEXT, port INTEGER,
  database_name TEXT, schema_name TEXT,
  auth_ref TEXT, read_only INTEGER,
  pool_config_json TEXT, status TEXT
);
```

### 3.2 节点状态机

```
PENDING → READY → RUNNING → COMPLETED
                    │
                    ├→ POLLING（轮询子状态，可多次迭代）
                    │      ├→ COMPLETED（条件满足）
                    │      └→ FAILED（超时）
                    │
                    └→ FAILED
                    
FAILED → 上游失败时，所有未执行后继标记 SKIPPED
```

### 3.3 凭据存储策略

- SQLite 中只存 `keychain://service/account` 或 `${env:VAR_NAME}` 引用。
- 真实值由本地程序从 OS 钥匙串或环境变量在运行时读取，**不经过模型上下文**。
- AI 扫描项目目录时，对疑似密码/密钥的值做占位替换（`****`），模型只看到“这里有一个凭据字段”。

---

## 4. 核心模块设计

### 4.1 声明式 DAG 资产

**节点类型**：

| 类型 | 职责 | 关键配置 |
|---|---|---|
| `http` | 接口调用 | method、path、headers、body 模板、变量引用 |
| `poll` | 异步轮询 | 检查条件、退避策略（间隔序列）、总超时 |
| `assert` | 断言 | 字段路径、操作符、期望值 |
| `db_query` | 数据库查询/断言 | 连接名、参数化查询、结果映射 |
| `subworkflow` | 子工作流 | skill_id、版本 pin、输入/输出绑定 |
| `cleanup` | 数据回收 | 按 `run_id` 回收，终态无条件执行 |

**变量引用**：`${node_id.output.field.path}`。执行前静态校验：被引用节点必须是前驱，字段在上游 schema 中存在。

**版本化**：每次激活产生新版本；父引用显式 pin 到 semver；旧版本仍可解析，避免破坏性变更。

### 4.2 确定性执行引擎

**执行流程**：

1. 拓扑排序驱动调度；无依赖后继同时发起，并行度受有界信号量控制。
2. 变量解析写入 `input_resolved_json` 供审计。
3. 轮询节点按退避策略执行，每次迭代写 `poll_trace`（时间戳、实际状态值、距期望差）。超时保留完整轨迹。
4. 子工作流通过帧栈实现，变量作用域隔离，输入/输出通过显式 binding 映射回父帧。静态环检测在保存时执行。
5. 清理节点作为 DAG 固定终态，**无论前序成功或失败都执行**。
6. 失败时先运行确定性分类器，产出 `failure_category` 和 `error_code`，再交由自主性策略决策。

**确定性保证**：同一份已激活 DAG + 同一环境配置，`--no-llm` 模式输出逐字节一致；轮询节点禁止裸 `sleep`，必须带退避和超时。

### 4.3 AI 生成层与校验闸门

**生成器**：输入自然语言描述 + 接口 schema + 业务规则上下文 + 已沉淀 Skill 契约 + 修正知识库偏好，输出**声明式 DAG 候选（JSON）**，不输出代码。

**确定性校验闸门**（规则驱动，不依赖模型）：

- 无环（拓扑排序可完成）。
- 边引用的节点存在。
- 变量引用只指向**前驱节点**，字段在上游 schema 中存在。
- 轮询条件可机械判定。
- 必填字段齐全。

不通过则退回修正或标记“需人工介入”，**绝不进入执行路径**。

**待审区**：通过校验的候选进入计划库待审状态，画布以半透明预览呈现，用户激活后才产生可执行新版本。

### 4.4 Skill 体系与 nsh 插件化

**Skill 工件包**：元数据（名称/版本/依赖）、类型化输入输出契约、声明式内部 DAG、自带断言。

**版本化**：与 DAG 同构。新版本激活时列出所有受影响 pin 引用，提示批量更新或逐个确认。

**nsh 插件**：Skill 可携带轻量 TSX 片段，用于自定义节点外观或详情面板（如轮询 Skill 显示“第 3/15 次 · 状态=PENDING”）。TSX 模块以签名包下发，在宿主配置的沙箱中加载，与 Rust 核心通过 `native_module` 类型化接口通信。

**复用机制**：当 AI 检测到用户多次手动搭建相似子链路时，主动建议“抽取为可复用 Skill”。Skill 库显示引用次数，作为复用性是否兑现的直观指标。

### 4.5 失败分类与自主性策略

**失败分类枚举**：

| 分类 | 判定依据 |
|---|---|
| `connection_error` | 连接拒绝/超时/DNS 失败 |
| `timeout` | 请求或轮询超时 |
| `assertion_failed_status` | 状态码不匹配 |
| `assertion_failed_body_shape` | 响应体结构/字段值不匹配 |
| `capture_error` | 上游断言通过但变量提取失败（响应形状漂移） |
| `business_rule_violation` | 业务规则冲突（如状态非法流转） |
| `unknown` | 无法归类 |

**自主性策略**：

| 分类 | 默认级别 | 行为 |
|---|---|---|
| `connection_error` / `timeout` | autonomous | 自动重试一次；仍失败标记环境问题，**不调 LLM** |
| `assertion_failed_*` | gated | LLM 生成候选修复，**必须在子进程中真实执行通过**才提交人工审 diff |
| `business_rule_violation` | manual | 转人工，不调 LLM |
| `unknown` | manual | 转人工 |

### 4.6 契约漂移检测

独立后台任务（可配置频率，默认每日一次）。对每个已激活接口做只读探测，计算响应形状指纹（字段路径集合 + 类型 + 嵌套深度），与最新记录 diff。

有差异则写 `drift_events`，反向查询引用该接口的 DAG 节点与 Skill，写 `drift_impacts`。**不自动修改 DAG**，只生成迁移候选进入待审区。

### 4.7 修正知识库

用户每次修改 AI 生成物（采纳前改、激活后改）记录 `diff_json` 和 `correction_type`，按 `pattern_key`（修正类型 + 目标标识 + 字段路径）聚合。

同一模式出现 ≥3 次时生成偏好建议。用户接受后**作为上下文注入后续 AI 生成的提示词**，从源头减少同类修正。拒绝则标记 `dismissed`，不再重复提示。

### 4.8 数据生命周期管理

数据工厂或条件查询 Skill 产出数据时同步登记 `test_data_records`（`run_id`、实体 ID、回收策略、状态）。

清理节点在执行终态运行，报告中汇总“使用 X / 回收 X / 残留 X”。残留数据在下次运行开始时提醒，并提供手动清理与重试入口。

### 4.9 连接配置与认证管理

**连接资产**：具名、类型、host/port/db、`auth_ref`（钥匙串引用）、`read_only` 标记、连接参数。密码不进 SQLite。

**AI 辅助发现**：用户主动触发、范围受限的只读扫描（`.env*`、`application*.yml`、`docker-compose*.yml`、`config/`、`*.toml`）。疑似密码做占位替换，不进入模型上下文。输出候选草稿列表（带置信度和来源文件），经用户确认后从本地读取真实凭据存入钥匙串。

**认证方案**：环境级定义（API Key/Bearer/Basic/OAuth 等），接口/场景级引用。值以 `${auth.bearerToken}` 变量在运行时注入。

**与 Skill 的衔接**：查询类 Skill 声明“需要一个只读数据库连接”，场景编排时从连接库选择绑定。切换环境时 Skill 自动使用该环境绑定的连接，无需修改 Skill。

### 4.10 接口导入管道

| 格式 | 解析策略 |
|---|---|
| OpenAPI / Swagger | 确定性解析器 |
| Postman Collection | 官方 Collection Schema 解析 |
| cURL / HAR | 请求还原 + 启发式推断 |
| Markdown | **AI 抽取 + 人工确认**：AI 输出结构化接口草稿（带置信度），用户逐条确认后归一化入库 |

Markdown 导入流程：AI 抽取 → 待审队列（预览卡片 + 置信度标注）→ 人工确认/修正 → 增量合并（合并/覆盖/跳过）→ 入库。

### 4.11 AI 调用预算与熔断

**预算三级**：全局 / Skill / 场景，各自配置每轮最大调用次数和成本上限。

**调用前检查**：预算余量 + `repair_success_rate`（某分类历史修复成功率低于阈值 10% 时跳过 LLM 调用）。

**调用后记录**：写 `llm_call_logs`，回写成功率统计。报告固定显示“AI 调用 X 次 · 预估成本 Y · 熔断 Z 次”。

### 4.12 CI/CD 确定性执行入口

CLI：`mytool run --scenario <id|tag> --env <name> --no-llm --gate --report junit.xml`。

`--no-llm` 为 CI 默认：执行路径完全确定性，不调任何 LLM；失败只分类、不生成修复。

门禁判定读 `gate_policies`：P0 场景通过率 < 100% 则 exit code 1；`block_on_drift` 为真且存在未处理漂移事件则 exit code 2。输出 JUnit XML 供 CI 消费，同时写回 `dag_runs`，桌面端可查看同一次运行详情。

---

## 5. 界面设计

### 5.1 整体布局

```
┌──────────────────────────────────────────────────────────────────────────┐
│ 顶栏：Logo │ 环境切换 dev/stg │ 运行▶ 停止■ 重跑失败 │ 搜索 │ AI副驾开关 │ ⚙ │
├────────────┬────────────────────────────────────────────┬────────────────┤
│            │  标签页：场景A × │ 场景B × │ 报告#123 ×        │                │
│  左导航     ├────────────────────────────────────────────┤  右上下文面板   │
│            │                                            │                │
│ ▸ 场景库    │           中 央 画 布 区                    │  节点属性       │
│ ▸ 计划库    │        （DAG 编辑 / 执行态 / 报告回放）       │  / 详情 / 校验   │
│ ▸ 报告      │                                            │                │
│ ▸ Skill库   │                                            │                │
│ ▸ 子计划    │                                            │                │
│ ▸ 环境      │                                            │                │
│            │                                            │                │
├────────────┴────────────────────────────────────────────┴────────────────┤
│ 底部抽屉：实时日志 │ 轮询轨迹 │ 变量监视 │ 控制台   （可折叠/可拖高）        │
├──────────────────────────────────────────────────────────────────────────┤
│ 状态栏：● 连接 SQLite │ 当前执行 3/12 节点 │ 并行度 2 │ SSE 已连接 │ 耗时 4.2s │
└──────────────────────────────────────────────────────────────────────────┘
```

默认尺寸：左导航 240px（可折叠到 48px 图标栏）、右面板 360px（可拖 240–560px）、底部抽屉 200px、顶栏 48px、状态栏 24px。

### 5.2 各区域元素

**顶栏**：环境选择器（带连通性状态点）、运行控制组（▶ 运行 / ■ 停止 / ⟳ 重跑失败节点）、运行模式切换（单次/循环/指定节点起跑）、全局搜索（Ctrl/Cmd+K）、AI 副驾开关、设置齿轮。

**左导航**：场景库（带最近执行状态点和通过率迷你条）、计划库（带校验状态徽标 ✓/⚠/✗）、报告列表、Skill 库（带引用次数）、子计划库、环境配置入口。支持拖拽子计划到画布、双击打开标签页。

**中央画布（三模式）**：

- **编辑视图**：节点卡片（类型图标 + 名称 + 一行摘要）；边带箭头，依赖边可标注变量传递；画布控件（模式切换、缩放、自动布局、网格、小地图）；节点操作（单击选中、双击编辑、右键菜单含“从此节点起跑”）；AI 生成入口（半透明预览节点，确认后落库）。
- **执行态视图**：节点状态色（灰=待执行、蓝脉动=执行中、绿=通过、红=失败、黄脉动=轮询中、深灰=跳过）；并行分支同时脉动；轮询节点显示 `第 3/15 次 · 状态=PENDING`；执行进度条 + 摘要；失败节点自动居中。
- **报告回放视图**：静态 DAG 叠加最终执行态；报告摘要条（通过率、耗时、与上次对比、触发方式）；时间轴滑块回放；失败节点“重跑此节点及下游”按钮。

**右上下文面板**（随选中对象切换）：

- 选中节点（编辑态）：属性表单（请求方法/路径、Headers、Body 模板带变量高亮和补全、断言编辑器、轮询配置）、上游可用变量列表。
- 选中节点（执行态/回放）：实际请求/响应、断言逐条结果带 diff、轮询轨迹、耗时分解。
- 选中节点（校验失败）：校验问题列表 + “交给 AI 修正”按钮。
- 未选中：场景元信息、历史通过率趋势。
- AI 副驾 Tab：对话式界面，每条输出绑定具体工件（插入画布/定位节点/采纳断言）；引用节点可点击定位；顶部显示 AI 当前上下文提示。

**底部抽屉（四 Tab）**：实时日志（按节点分组、可过滤）、轮询轨迹（表格：节点/第几次/时间戳/实际状态/距期望值）、变量监视（实时更新已解析变量）、控制台（手动发请求/查 DB/临时断言）。

**状态栏**：SQLite 连接状态、执行摘要（`3/12 节点 · 2 并行 · 1 轮询中`）、SSE 连接状态、当前环境、总耗时、快捷键提示。

### 5.3 关键交互

1. **节点级重跑**：失败节点右键“从此节点重跑”，利用已存档上游输出作为上下文，仅该节点及其下游重新执行。
2. **AI 生成物闸门**：生成物先进入计划库待审区，画布半透明预览，校验徽标明确显示 ✓/⚠/✗，用户点“激活”后才可执行。
3. **失败即聚焦**：执行中一旦失败，画布自动居中失败节点、右面板切到失败详情、底部日志过滤到该节点。
4. **布局持久化**：每个场景记住画布缩放/位置、面板宽度、底部抽屉状态，存 SQLite。
5. **快捷键**：`Ctrl/Cmd+K` 全局搜索、`Ctrl/Cmd+Enter` 运行当前、`Ctrl/Cmd+Shift+Enter` 重跑失败、`F` 适应画布、`1/2/3` 切模式、`?` 快捷键面板。

### 5.4 工作区预设

- **编排者**：DAG 编辑视图 + 右属性面板 + AI 副驾，底部抽屉折叠。
- **执行/调试者**：执行态视图 + 右详情面板 + 底部轮询轨迹/日志展开。
- **看报告者**：报告列表 + 回放视图 + 历史对比面板。

预设存 SQLite，一键切换。

---

## 6. 关键业务流程

### 6.1 接口导入流程

```
选择格式 → 解析（确定性/AI抽取）→ 预览卡片（置信度标注）
→ 人工确认/修正 → 增量合并（合并/覆盖/跳过）→ 归一化入库
```

Markdown 导入时 AI 输出结构化草稿，低置信度项黄色高亮。

### 6.2 AI 编排流程

```
自然语言描述 + 接口 schema + 业务规则 + Skill 契约 + 修正偏好
→ AI 生成声明式 DAG 候选（JSON）
→ 确定性校验闸门（环/前驱引用/schema字段/轮询可判定）
→ 通过：进入待审区（半透明预览）
→ 不通过：退回修正或标记“需人工介入”
→ 用户激活 → 产生可执行新版本
```

### 6.3 场景执行流程

```
加载已激活 DAG → 拓扑排序 → 调度（并行分支同时发起）
→ 变量解析（写入审计）→ 节点执行
→ 轮询节点：退避迭代 + 轨迹记录
→ 子工作流：帧栈 + 作用域隔离
→ 失败：确定性分类器 → 自主性策略决策
→ 清理节点：无条件执行数据回收
→ 报告生成（DAG 回放 + 节点详情 + 成本/数据汇总）
```

### 6.4 失败处理流程

```
节点失败 → 确定性分类器（正则+状态码+异常类型）
→ 产出 failure_category + error_code
→ 查 autonomy_policy：
   connection_error/timeout → 自动重试（不调 LLM）
   assertion_failed_* → LLM 生成候选修复 → 子进程验证 → 人工审 diff
   business_rule_violation/unknown → 转人工
→ 调用前检查 llm_budget + repair_success_rate（熔断）
→ 记录 llm_call_logs，回写成功率
```

### 6.5 数据复用流程

```
自然语言条件（"VIP客户，余额>500"）
→ LLM 翻译为结构化查询意图（缓存 L1：条件文本+schema哈希+提示词版本）
→ Skill 模板匹配（90% 走模板，10% LLM 兜底）
→ 只读数据库查询（L2 缓存：按 run_id 隔离，短 TTL）
→ 得到用户 ID → 认证 Skill 登录（token 复用）
→ 执行业务场景
→ 登记 test_data_records → 清理节点回收
```

### 6.6 自动校准流程

```
DAG 候选 → 确定性校准（结构/引用/参数，无人值守，可自动修复）
→ 语义校准（LLM 生成候选修复 diff）
→ 待审区预览 → 用户一键采纳
→ 新一轮确定性校准自动运行通过 → 激活
```

---

## 7. 开发计划

### 7.1 阶段划分与里程碑

| 阶段 | 周期 | 目标 | 验收 |
|---|---|---|---|
| **阶段 0：骨架与引擎** | 4–6 周 | 手工 DAG 在确定性引擎跑通串行/并行/轮询/子工作流/清理 | 同一 DAG 两次 `--no-llm` 执行断言逐字节一致，无 LLM 调用；轮询超时轨迹完整可回放 |
| **阶段 1：桌面界面与可视化** | 4–5 周 | 编辑/运行/调试 DAG | 排查轮询超时只需点失败节点，右面板即显示完整轨迹和 diff |
| **阶段 2：AI 生成与校验闸门** | 4–5 周 | AI 候选经闸门和待审区进入可执行状态 | 含无效变量引用的候选被拦截并定位；通过候选激活后行为与手工一致 |
| **阶段 3：Skill 体系与 nsh 插件** | 3–4 周 | 高频子链路沉淀为 Skill，可携带 TSX 扩展 | 同一 Skill 换条件不需改 DAG；新版本激活列出所有受影响 pin |
| **阶段 4：六缺口收束与 CI 嵌入** | 4–5 周 | 漂移/分类策略/修正闭环/门禁/熔断/数据回收全部闭环 | 上游删字段在执行前标橙；`connection_error` 不触发 LLM；失败执行后数据残留为 0 |

**总计约 19–25 周**（三条并行线可压缩关键路径）。

### 7.2 起跑线：三条并行线（前 2 周）

**线 A：引擎语义 spike（最高优先级）**

- 选 3 条真实链路：纯串行、含并行分支（c∥d）、含异步轮询（e 等支付回调）。
- 引擎只支持 `http` / `poll` / `assert` + 变量绑定 + 失败分类器。
- 验证三个语义问题：变量解析时机是否够用；轮询退避在真实接口上是否 flaky；失败分类器正则覆盖率。

**线 B：UI 骨架在 Web 目标先跑（与 A 并行）**

- 用 TSX + Dagre 在 Vite + React 上做画布三模式 + 右面板 + 底部轮询轨迹，数据源读线 A 的 SQLite。
- 组件设计为渲染目标无关（纯数据 props），迁移 nsh 只换外壳。
- 验证交互模型：“不翻日志定位失败”是否成立。

**线 C：提示词与闸门原型（最轻量）**

- 手工构造 20 个坏 DAG（有环/无效引用/轮询不可判定/孤立节点），确认闸门 100% 拦截。
- 用 3 条链路描述 + schema 测试模型输出，重点看格式稳定性和字段命名可预期性。

### 7.3 两周联合评审（只看三个问题）

1. 线 A 轮询链路连续 10 次执行是否 10 次稳定？抖动是引擎问题还是被测系统问题？
2. 线 B 失败节点从“变红”到“人在右面板看到完整证据”是否在 2 次点击内完成？
3. 线 C 20 个坏 DAG 是否 100% 拦截？模型输出被闸门接受比例是否 ≥ 70%？

---

## 8. 验收标准

### 8.1 五个核心验收问题

| # | 问题 | 对应模块 |
|---|---|---|
| 1 | 同一份已激活 DAG 在 `--no-llm` 下连续两次 CLI 执行，断言结果逐字节一致且无 LLM 调用记录？ | 确定性执行与 CI 嵌入 |
| 2 | 一个 `timeout` 失败没有触发 LLM 调用，而是按策略自动重试或标记环境问题；一个 `assertion_failed` 的候选修复在提交前真实跑通过？ | 失败分类与熔断 |
| 3 | 上游接口 schema 变更在测试执行失败之前就被漂移检测捕获并标注到具体节点？ | 前置发现 |
| 4 | 用户对 AI 生成物的同类修正第三次出现时被识别并建议固化，接受后新生成物不再犯同样错误？ | 反馈闭环 |
| 5 | 一轮无论成功或失败的执行结束后，AI 调用次数/成本可见，测试数据回收状态可见，残留数据在下次运行前被提醒？ | 成本与数据卫生可观测性 |

### 8.2 持续度量看板

从第一天起在 CI 中埋入验收看板，每次执行自动计算：确定性一致率、timeout 的 LLM 调用次数、漂移前置发现数、修正复用率、数据回收残留数。

### 8.3 分阶段验收（摘录）

| 阶段 | 验收问题 |
|---|---|
| 0 | 同一 DAG 两次 `--no-llm` 执行断言逐字节一致；轮询超时轨迹完整可回放 |
| 1 | 排查一次轮询超时失败，点失败节点右面板即显示完整轨迹和断言 diff |
| 2 | 含无效变量引用的 AI 候选入库前被拦截并定位；通过候选激活后行为与手工一致 |
| 3 | 同一条件查询 Skill 更换条件不需改 DAG；Skill 新版本激活列出所有受影响 pin |
| 4 | 上游删字段执行前标橙；`connection_error` 不触发 LLM；失败执行后数据残留为 0 |

---

## 9. 风险与应对

| 风险 | 应对 |
|---|---|
| AI 生成物“语法正确但业务错误” | 校验闸门只拦结构错误；业务断言必须人工确认后激活。Skill 模板覆盖 90% 常见场景，LLM 只走兜底 |
| nsh 桌面端成熟度 | 阶段 0–1 先用 Web 目标验证交互，nsh 打包并行；引擎和 CLI 完全独立于 nsh |
| 轮询导致 flaky | 轮询节点必须带退避和超时，轨迹完整存档；禁止裸 `sleep` |
| LLM 成本失控 | 预算表 + 成功率熔断 + 报告成本可见；历史成功率低的分类直接跳过 LLM |
| 数据污染 | 只读连接优先用于查询；写操作走 `run_id` 隔离；清理节点无条件执行 |
| 团队对 AI 产物信任侵蚀 | 待审区 + 半透明预览 + 校验徽标 + 修正知识库全程可审计 |
| 验收标准随规模退化 | CI 埋验收看板持续度量五个指标 |

---

## 10. 附录

### 10.1 术语表

| 术语 | 定义 |
|---|---|
| DAG | 有向无环图，节点为操作，边为依赖 |
| Skill | 自包含的可复用能力单元（元数据 + 契约 + 内部 DAG + 断言） |
| 校验闸门 | 确定性规则集，拦截 AI 生成物的结构性错误 |
| 待审区 | AI 候选在激活前的暂存状态 |
| 自主性级别 | manual / gated / supervised / autonomous 四档 |
| 漂移 | 接口实际响应形状与记录指纹不一致 |
| 帧栈 | 子工作流执行时的变量作用域栈 |

### 10.2 参考项目

- **hexdag**：声明式 YAML + DAG 执行模型、async-first、六边形架构
- **Dagre**：DAG 自动布局
- **nsh**：Rust + TSX 桌面框架、沙箱 TSX 插件
- **MASTEST**：LLM 生成测试脚本 + 人工审查修正
- **ATI**：失败分类 + 自主性旋钮 + Bayesian Flaky Detector
- **Tarn**：稳定 failure_category 枚举 + error_code
- **SpecGuard**：响应形状指纹 + 漂移告警
- **Voiden / ReqBook**：Markdown 作为 API 资产一等公民
- **Proba / Touchstone**：SQLite + 文件系统本地优先、SSE 实时看板

### 10.3 提示词模板示例（AI 生成 DAG）

```
你是接口测试编排专家。根据以下信息输出声明式 DAG 的 JSON。

【接口 schema】{openapi_json}
【业务规则】{business_rules}
【可用 Skill 契约】{skill_contracts}
【历史修正偏好】{preference_rules}

要求：
1. 只输出 JSON，不输出解释。
2. 节点类型仅限：http / poll / assert / db_query / subworkflow / cleanup。
3. 变量引用格式 ${node_id.output.field}，被引用节点必须是前驱。
4. 轮询节点必须声明 condition、backoff（间隔序列）、timeout。
5. 每个 http 节点必须附带至少一条业务断言。
6. 输出必须能通过以下校验：无环、边引用节点存在、变量引用字段在上游 schema 中存在。
```

---

**文档状态**：设计定稿，可进入阶段 0 开发。三条并行起跑线于第 1 周启动，两周后按 7.3 节三个问题联合评审，通过后按阶段 0–4 推进。
