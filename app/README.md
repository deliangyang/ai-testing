# NSH desktop shell (P1-31)

Phase 1 UI lives in [`web/`](../web/) (Vite + React). The desktop path embeds that bundle in a
WebView and wires **`DataSource`** through Rust `#[module] DataSource` in `app/aitest/native`.

## Prerequisites

- [NSH installation](https://nsh.dev/docs/installation.md) (`nsh` on `PATH`, Windows 10 1809+)
- Rust toolchain (same as the repo workspace)
- Built web assets: `cd web && npm run build`

## Layout (`app/aitest/`)

```
app/aitest/
  nsh.json          # mixed app: rust = native, root = App.tsx
  App.tsx           # WebView host → web/dist?api_base=<embedded server>
  App.test.tsx      # `nsh test` contract smoke
  native/           # #[native_module("aitest/native")] → nsh-bridge
```

## Data flow

1. `DataSource.ensure_api()` (`native`) calls `nsh-bridge::ensure_default_api()` and starts an
   embedded `server` listener on `127.0.0.1:*`.
2. `DataSource.web_shell_url()` points the WebView at `web/dist/index.html?api_base=…`.
3. The web bundle reads `api_base` via [`web/src/host/apiBase.ts`](../web/src/host/apiBase.ts) and
   can use [`NshDataSource`](../web/src/data/nsh.ts) (delegates to `HttpDataSource` for SSE/writes).

Without the NSH CLI, CI still validates the Rust side: `cargo test -p nsh-bridge`. To compile the
native UI set locally: `cd app/aitest/native && cargo check --features nsh`.

## Commands

| Step | Command | Proves |
|------|---------|--------|
| Rust bridge + host | `cargo test -p nsh-bridge` | Store + embedded API + sync reads |
| Web contract | `cd web && npm test` | `NshDataSource` + fixture/http contract |
| NSH headless | `scripts/nsh-spike.ps1` | `nsh test app/aitest` when CLI installed |

Environment:

- `AITEST_DB` / `AITEST_ENV_ROOT` — SQLite path (same as `aitest-server`)
- `AITEST_WEB_DIST` — override path to `index.html` (default `{AITEST_REPO_ROOT}/web/dist/index.html`)
- `AITEST_REPO_ROOT` — repo root for default `web/dist` resolution in Rust

See [ADR-006](../docs/decisions/ADR-006-nsh-desktop-spike.md).
