# NSH desktop shell (spike → P1-31)

This directory holds the **Native Shell (NSH)** app scaffold for ai-testing. Phase 1 UI
lives in [`web/`](../web/) (Vite + React); the desktop path embeds that UI and wires
`DataSource` through a Rust `native_module`.

## Prerequisites

- [NSH installation](https://nsh.dev/docs/installation.md) (`nsh` on `PATH`, Windows 10 1809+)
- Rust toolchain (same as the repo workspace)
- Built web assets: `cd web && npm run build`

## Spike checklist (P1-30)

| Step | Command | Proves |
|------|---------|--------|
| Rust bridge | `cargo test -p nsh-bridge` | `store` reachable from desktop Rust surface |
| NSH headless | `scripts/nsh-spike.ps1` | `nsh test` runs without a window (when CLI installed) |
| Mixed app | `nsh test app/aitest` | TSX entry + `native_module` wiring (after `nsh init` merge) |

## Planned `aitest` layout (mixed template)

```
app/aitest/
  nsh.json          # "rust": ["native"], root TSX or Rust per embed mode
  App.tsx           # host: WebView → file:// or http://127.0.0.1:<server>
  App.test.tsx      # nsh test
  native/           # #[native_module("aitest/native")] → nsh-bridge APIs
```

See [ADR-006](../docs/decisions/ADR-006-nsh-desktop-spike.md) for conclusions and alternatives.
