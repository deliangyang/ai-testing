# AGENTS.md

本文件约束所有在本仓库中工作的 AI agent（及人类开发者）。目标：按 [`TASKS.md`](./TASKS.md) 逐个完成原子任务，把 [`develop.md`](./develop.md) 描述的 AI 测试系统做出来。

## 1. 文件角色

| 文件 | 角色 | 谁可以改 |
|---|---|---|
| `develop.md` | 设计权威。回答"系统应该是什么样" | 只有人类；agent 发现设计问题时登记到 TASKS.md「待决问题」，不直接改 |
| `TASKS.md` | 进度权威。回答"下一步做什么、做到哪了" | agent 每个任务开始/结束时必须更新 |
| `AGENTS.md` | 工作方式权威。回答"怎么做" | 人类；agent 可提议修改 |
| `docs/decisions/ADR-###-*.md` | 实现层面的技术决策记录 | agent 在做出设计文档未覆盖的决定时新建 |
| `docs/reviews/*.md` | 里程碑与阶段验收记录 | 对应的验收任务 |

发生冲突时的优先级：人类在对话中的指令 > `develop.md` > `AGENTS.md` > `TASKS.md` > 现有代码。

## 2. 工作循环（每个会话都照此执行）

1. **定位**：读 `TASKS.md`。如果有 `DOING` 任务，先继续它；否则按第 3 节规则选下一个任务。
2. **认领**：把任务状态改为 `DOING`，在变更记录追加一行。
3. **读设计**：读任务涉及的 `develop.md` 章节，以及依赖任务产出的代码与测试。不要凭记忆假设接口。
4. **先写测试**：把任务的「验收标准」翻译成失败的测试（单测、集成测试或快照测试），再实现。
5. **实现**：只做本任务范围内的事。发现必须先做的前置工作时，在 TASKS.md 插入新任务（如 `P0-33a`）并调整依赖，不要顺手做。
6. **检查**：运行 `./scripts/check.ps1`（P0-02 完成前用 `cargo fmt --check; cargo clippy --workspace -- -D warnings; cargo test --workspace`），必须全绿。
7. **收尾**：任务状态改为 `DONE`，在变更记录写明提交哈希和与设计的偏差（如有）。提交信息用英文，格式：`[P0-15] gate: add acyclicity rule reporting the cycle path`，正文简述做了什么、如何验证、与设计的偏差。
8. **一次一个任务**：一个任务对应一次提交（或一个 PR）。完成后如果会话还要继续，回到第 1 步。

## 3. 选任务规则

按顺序判断：

1. 状态为 `TODO`，且「依赖」列中的任务全部为 `DONE`。
2. **阶段门**：阶段 N 的验收任务（如 `P0-57`）为 `DONE` 之前，不开始阶段 N+1 的任务。例外：TASKS.md「起跑线映射」中列出的线 B 任务可以在阶段 0 并行推进。
3. 在满足条件的任务中，取阶段最小、序号最小的那个。
4. 没有可选任务时（全部被阻塞），停下来向人类报告阻塞链和需要的决策，不要绕开。

## 4. 仓库布局（目标结构）

```
ai-testing/
├── AGENTS.md  TASKS.md  develop.md
├── Cargo.toml                  # workspace
├── crates/
│   ├── common/                 # 错误约定、tracing、Clock/Sleeper/IdGen
│   ├── model/                  # DAG 类型、变量引用解析、规范化 JSON
│   ├── gate/                   # 确定性校验闸门 + 自动校准
│   ├── store/                  # SQLite、迁移、Repository
│   ├── secrets/                # keychain:// 与 ${env:} 引用解析
│   ├── classifier/             # 失败分类（纯规则）
│   ├── engine/                 # 调度、执行器、帧栈、事件流
│   ├── importer/               # OpenAPI/Postman/cURL/HAR/Markdown/目录扫描
│   ├── llm-gateway/            # Provider、脱敏、日志、预算、熔断
│   ├── generator/              # 提示词组装、DAG 候选生成、修正知识库
│   ├── skills/                 # Skill 包、版本、影响分析、插件包
│   ├── drift/                  # 指纹、漂移检测、影响分析
│   ├── autonomy/               # 自主性策略（唯一能把失败和 LLM 连起来的地方）
│   ├── server/                 # 本地 HTTP API + SSE（Web 目标）
│   ├── cli/                    # mytool 二进制
│   └── mock-sut/               # 模拟被测系统，仅测试使用
├── web/                        # Vite + React + TS 前端（渲染目标无关组件）
├── app/                        # nsh 外壳（P1-30 之后）
├── prompts/  schemas/  skills/  plugins/  examples/
├── tests/acceptance/           # §8.1 五个核心验收问题
├── docs/{decisions,reviews}/   # ADR、评审与验收记录
└── scripts/check.ps1           # 唯一的质量检查入口
```

crate 依赖方向（不允许反向或横向越级）：

```
common ← model ← gate
common ← store
model, store, classifier, common ← engine
engine, gate, store ← cli（默认构建不含 llm-gateway / autonomy / generator）
secrets ← llm-gateway ← generator ← autonomy、drift、importer(Markdown)
engine, store, gate, generator, autonomy, drift, skills ← server
```

## 5. 红线（违反即任务不算完成）

这些规则来自 `develop.md` §1.3 的设计原则，能自动检查的都要由 `scripts/check.ps1` 检查。

1. **确定性执行不依赖模型**：`engine`、`gate`、`classifier`、`store` 以及 `cli` 的默认构建，依赖树中不得出现 `llm-gateway`/`generator`/`autonomy`。（检查：`cargo tree -p <crate>`）
2. **禁止裸 sleep**：只有 `common` 中的 `Sleeper` 实现可以调用 `std::thread::sleep`/`tokio::time::sleep`。轮询必须有退避序列和总超时。（检查：rg 搜索）
3. **确定性输出**：时间、ID、随机数一律通过 `Clock`/`IdGen`/带 seed 的 RNG 注入；报告与快照使用规范化 JSON；并行完成顺序不得影响输出。不要遍历 `HashMap` 生成输出，用 `BTreeMap` 或显式排序。
4. **AI 产物永不直接生效**：任何由 LLM 产生的 DAG/Skill/接口定义只能以 `pending`/`draft` 状态写入；只有经过闸门的显式激活操作才能把它变成 `active`。不得新增绕过这条路径的 API。
5. **凭据不进 SQLite、不进模型、不进日志**：数据库只存 `keychain://...` 或 `${env:...}` 引用；所有发往 LLM 的内容经过 Redactor；执行快照与日志中的认证值显示为 `****`。
6. **失败必须分类**：每个 FAILED 节点都要有 `failure_category` 和 `error_code`，分类只用确定性规则。
7. **测试不访问外网、不调真实 LLM**：HTTP 用 wiremock 或 `mock-sut`；LLM 用录制回放 Mock。真实调用的测试必须用显式环境变量开启（如 `AITEST_LIVE_LLM=1`），默认跳过。

## 6. 编码约定

**Rust**
- edition 2021，stable 工具链；`clippy -D warnings`；库代码不用 `unwrap`/`expect`（测试和明确不可达处除外，并写明原因）。
- 错误：库 crate 用 `thiserror` 定义具体错误类型，`anyhow` 只在 `cli`/`server` 的顶层使用。
- 异步运行时：tokio。HTTP 客户端：reqwest。SQLite：rusqlite（引擎同步写入放在 `spawn_blocking`）。
- 公共类型 derive `Serialize, Deserialize, Debug, Clone, PartialEq`；需要导出 schema 的同时 derive `JsonSchema`。
- 数据库变更只能通过新增迁移文件，不修改已提交的迁移。

**TypeScript / 前端**
- TS strict；组件只接收纯数据 props，数据访问统一走 `DataSource` 接口，不在组件里直接 fetch。
- 后端类型从 Rust 生成（P1-02），不手写重复定义。
- 状态色、尺寸等设计常量集中在一个 tokens 文件，取值以 `develop.md` §5 为准。

**通用**
- 注释只写代码本身表达不了的约束，不写"这行做什么"。
- 新增依赖要有理由：优先使用已在 workspace 中的依赖；引入新 crate/npm 包时在提交说明里写一句为什么。

## 7. 测试约定

| 类型 | 位置 | 说明 |
|---|---|---|
| 单元测试 | 模块内 `#[cfg(test)]` | 表驱动优先 |
| 集成测试 | `crates/<name>/tests/` | 跨模块、带 SQLite（临时文件）或 wiremock |
| 坏 DAG 语料 | `crates/gate/tests/fixtures/bad_dags/` | 每个文件头注明期望 issue code |
| 快照测试 | insta | 用于解析器输出、报告、schema；快照变更须在提交说明中解释 |
| 确定性 golden | `crates/cli/tests/` | 同一 DAG 两次运行确定性报告逐字节一致 |
| 前端 | Vitest + Testing Library；E2E 用 Playwright | E2E 使用 fixture 数据源或内嵌 mock-sut |
| 验收 | `tests/acceptance/` | 对应 §8.1 的五个问题 |

## 8. 完成定义（DoD）

一个任务只有同时满足以下条件才能标 `DONE`：

- TASKS.md 中该任务的验收标准全部有对应的自动化测试，并且通过。
- `scripts/check.ps1` 全绿，没有新增 warning。
- 没有违反第 5 节红线。
- 公开接口变化已同步到生成的 schema/TS 类型。
- 对 `develop.md` 的任何偏离都写进了变更记录；属于技术决策的另写 ADR。
- TASKS.md 状态与变更记录已更新，并且和代码在同一次提交中。

## 9. 遇到不确定时

- **设计没写、影响小、可逆**（如内部命名、库选型细节）：自行决定，在提交说明里写一句。
- **设计没写、影响接口或数据模型**：写 ADR（背景 / 选项 / 决定 / 后果），然后继续。
- **设计有矛盾或需要业务判断**：登记到 TASKS.md「待决问题」。如果有合理默认值，按默认值推进并在问题里写明；没有就把任务标 `BLOCKED`，换下一个任务。
- **任务太大**（预计超过一次提交能讲清楚的范围）：拆成 `a/b/c` 子任务写回 TASKS.md，再从第一个开始。
- **发现已完成任务有缺陷**：新增修复任务（如 `P0-35-fix1`），不要悄悄改掉已验收的行为。

## 10. 阶段验收任务的做法

阶段验收任务（`P0-57`/`P1-32`/`P2-27`/`P3-22`/`P4-32`）和 M0 评审（`P0-56`）不写新功能，只做：

1. 逐条复核该阶段的验收标准（`develop.md` §7.1 / §8.3），每条给出可复现的命令或测试名作为证据。
2. 运行全量检查和 E2E。
3. 写 `docs/reviews/phaseN.md`：结论、证据、遗留问题（遗留问题转成下一阶段的新任务）。
4. 任一条不满足时，验收任务保持 `DOING`，新增修复任务并先完成修复。

## 11. 常用命令

```powershell
./scripts/check.ps1                         # 全量质量检查（提交前必跑）
cargo test -p gate                          # 单个 crate 测试
cargo run -p mock-sut                       # 启动模拟被测系统
cargo run -p cli -- dag validate examples/serial.yaml
cargo run -p cli -- run --dag examples/async-poll.yaml --env local --no-llm --report-json out/report.json
cd web; npm run dev                         # 前端开发（fixture 数据源）
cd web; npm test; npx playwright test       # 前端单测 / E2E
```

命令在对应任务完成后才可用；以 `scripts/check.ps1` 和各 crate 的 README 为准。
