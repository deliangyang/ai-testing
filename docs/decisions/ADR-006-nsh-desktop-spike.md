# ADR-006：NSH 桌面外壳技术 spike（P1-30）

- 状态：已采纳（有条件）
- 任务：P1-30
- 日期：2026-11-09

## 背景

`develop.md` 将 **NSH**（[nsh.dev](https://nsh.dev/)）定为桌面框架：Rust 核心承载 engine/store/钥匙串，TSX 负责画布与面板，TSX 通过 `#[native_module]` 调用 Rust。阶段 1 已在 **Web**（Vite + React）验证 UI；需在引入 `app/` 前确认 NSH 工具链、无界面测试与现有 crate 的接法。TASKS「待决问题」Q1 询问 NSH 获取方式与 API 文档。

## Spike 证据

| 项 | 方法 | 结果 |
|----|------|------|
| 框架与 API | 官方文档 [Building with Rust](https://nsh.dev/docs/rust.md)、[`native_module`](https://docs.rs/nsh-macros/latest/nsh_macros/attr.native_module.html) | **可用**；混合模板（Rust + TSX）、类型生成到 `.nsh/types`，`nsh check` / 编辑器校验 TS 边界 |
| `store` 从桌面 Rust 调用 | 新建 `crates/nsh-bridge`，`list_dags(path)` 集成测试 | **通过**（`cargo test -p nsh-bridge`） |
| 复用现有 React 组件 | 组件纯 props + `DataSource` 接口（P1-01+） | **不必重写在 NSH TSX**；桌面壳加载 `web/dist`（WebView / 内嵌本地 `server`）即可满足阶段 1 |
| `nsh test` 无界面 | 文档 + `app/aitest/App.test.tsx` 占位；`scripts/nsh-spike.ps1` | **依赖本机安装 `nsh` CLI**；CI 默认跳过，安装后跑 `nsh test app/aitest` |
| 本机 CLI | `irm https://nsh.dev/install.ps1 \| iex`（Windows） | 安装包来自 GitHub release；需新开终端刷新 `PATH` |

## 决定

1. **继续以 NSH 为 P1-31 目标外壳**（与设计一致），不改为 Electron/Tauri 作为主路径。
2. **P1-31 集成形态**：
   - **UI**：保留 `web/` 构建产物；NSH TSX 壳内嵌 WebView，指向内嵌 `server` crate 或静态 `file://` + API（与浏览器同源策略需在壳层配置）。
   - **数据**：`crates/nsh-bridge` 扩展为 `#[native_module("aitest/native")]` 实现体，暴露与 `DataSource` 对齐的 JSON/RPC（先只读 API，写操作与 SSE 复用 `server` 逻辑）。
3. **仓库布局**：`app/aitest/` 使用 NSH **mixed** 模板；`native/` crate 依赖 workspace 的 `nsh-bridge`（路径依赖），由 `nsh init` / 手工合并 `nsh.json`。
4. **CI**：不强制安装 NSH；`nsh-bridge` 测试纳入 `cargo test --workspace`；`scripts/nsh-spike.ps1` 供本地与可选手动 job。

## 替代方案（未采纳为主路径）

| 方案 | 优点 | 缺点 |
|------|------|------|
| **Tauri / Wry + 现有 web** | 生态成熟、CI 简单 | 与 `develop.md` 选定 NSH 不一致；插件/Skill TSX 沙箱需另设计 |
| **仅 Web 目标** | 零桌面依赖 | 无法交付阶段 1「桌面」验收中的本机 SQLite/钥匙串一体化 |
| **整体迁到 NSH TSX 重写 UI** | 单栈原生 | 重复阶段 1 工作；违背「渲染目标无关、只换外壳」 |

## 后果

- **P1-31** 可开工：实现 `native_module` 注册、`DataSource` 的 NSH 实现、WebView 加载 `web` 构建。
- **P3-18** 插件沙箱仍跟 NSH 签名包模型对齐。
- **Q1 闭合**：NSH 从 https://nsh.dev/docs/installation.md 安装；版本与 `nsh-core` 以本机 `nsh --version` 为准；API 见 https://nsh.dev/docs/ 。
- **风险**：Windows CI 若需 `nsh test`，须缓存 NSH 分发包；Android/iOS 非阶段 1 必需。

## 验证

```powershell
cargo test -p nsh-bridge
./scripts/nsh-spike.ps1   # 安装 nsh 后
```
