import { useCallback, useEffect, useRef, useState } from 'react'

import { DagEditCanvas } from '../canvas'
import { SERIAL_DAG, SERIAL_EXECUTIONS } from '../data/fixtures/serial'
import { ExecutionDetailPanel } from '../panels'
import { clampBottomDrawerHeight, clampRightPanelWidth, leftNavWidthPx } from './layout'
import { shellTokens } from './tokens'
import './shell.css'

export function AppShell() {
  const [navCollapsed, setNavCollapsed] = useState(false)
  const [rightWidth, setRightWidth] = useState<number>(shellTokens.rightPanelWidthPx)
  const [drawerHeight, setDrawerHeight] = useState<number>(shellTokens.bottomDrawerHeightPx)
  const [drawerCollapsed, setDrawerCollapsed] = useState(false)

  const rightDrag = useRef<{ startX: number; startWidth: number } | null>(null)
  const drawerDrag = useRef<{ startY: number; startHeight: number } | null>(null)

  const endDrag = useCallback(() => {
    rightDrag.current = null
    drawerDrag.current = null
  }, [])

  useEffect(() => {
    const onMove = (event: PointerEvent) => {
      if (rightDrag.current) {
        const delta = rightDrag.current.startX - event.clientX
        setRightWidth(clampRightPanelWidth(rightDrag.current.startWidth + delta))
      }
      if (drawerDrag.current && !drawerCollapsed) {
        const delta = drawerDrag.current.startY - event.clientY
        setDrawerHeight(clampBottomDrawerHeight(drawerDrag.current.startHeight + delta))
      }
    }
    const onUp = () => {
      endDrag()
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
    return () => {
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
    }
  }, [drawerCollapsed, endDrag])

  const effectiveDrawerHeight = drawerCollapsed
    ? shellTokens.bottomDrawerCollapsedHeightPx
    : drawerHeight

  return (
    <div
      className={`app-shell${navCollapsed ? ' app-shell--nav-collapsed' : ''}`}
      data-testid="app-shell"
    >
      <header className="app-shell__top" role="banner">
        <strong>AITest</strong>
        <span className="app-shell__nav-label">环境 local</span>
        <span style={{ marginLeft: 'auto' }}>运行 · 搜索 · 设置</span>
      </header>

      <div className="app-shell__body">
        <nav
          className="app-shell__left"
          aria-label="主导航"
          data-testid="left-nav"
          style={{ width: `${String(leftNavWidthPx(navCollapsed))}px` }}
        >
          <button
            type="button"
            className="app-shell__nav-toggle"
            aria-expanded={!navCollapsed}
            aria-label={navCollapsed ? '展开左导航' : '折叠左导航'}
            onClick={() => {
              setNavCollapsed((v) => !v)
            }}
          >
            {navCollapsed ? '»' : '«'}
          </button>
          <ul className="app-shell__nav-list">
            <li>
              <span className="app-shell__nav-label">场景库</span>
            </li>
            <li>
              <span className="app-shell__nav-label">计划库</span>
            </li>
            <li>
              <span className="app-shell__nav-label">报告</span>
            </li>
            <li>
              <span className="app-shell__nav-label">Skill 库</span>
            </li>
          </ul>
        </nav>

        <div className="app-shell__center-col">
          <div className="app-shell__center-main">
            <section className="app-shell__center" aria-label="工作区">
              <div className="app-shell__tabs" role="tablist">
                <span role="tab" aria-selected>
                  场景 serial
                </span>
              </div>
              <main className="app-shell__canvas" data-testid="center-canvas">
                <DagEditCanvas dag={SERIAL_DAG} />
              </main>
            </section>

            <Resizer
              label="调整右面板宽度"
              className="app-shell__resize-v"
              onDragStart={(clientX) => {
                rightDrag.current = { startX: clientX, startWidth: rightWidth }
              }}
            />

            <aside
              className="app-shell__right"
              aria-label="上下文面板"
              data-testid="right-panel"
              style={{ width: `${String(rightWidth)}px` }}
            >
              <ExecutionDetailPanel
                nodeId="pay"
                execution={SERIAL_EXECUTIONS.find((e) => e.node_id === 'pay') ?? null}
              />
            </aside>
          </div>

          <Resizer
            label="调整底部抽屉高度"
            className="app-shell__resize-h"
            hidden={drawerCollapsed}
            onDragStart={(clientY) => {
              drawerDrag.current = { startY: clientY, startHeight: drawerHeight }
            }}
          />

          <section
            className="app-shell__drawer"
            aria-label="底部抽屉"
            data-testid="bottom-drawer"
            style={{ height: `${String(effectiveDrawerHeight)}px` }}
          >
            <div className="app-shell__drawer-toolbar">
              <span>日志 · 轮询 · 变量 · 控制台</span>
              <button
                type="button"
                className="app-shell__drawer-toggle"
                aria-expanded={!drawerCollapsed}
                aria-label={drawerCollapsed ? '展开底部抽屉' : '折叠底部抽屉'}
                onClick={() => {
                  setDrawerCollapsed((v) => !v)
                }}
              >
                {drawerCollapsed ? '展开' : '折叠'}
              </button>
            </div>
            {!drawerCollapsed ? <div className="app-shell__drawer-body">底部抽屉内容</div> : null}
          </section>
        </div>
      </div>

      <footer className="app-shell__status" role="contentinfo" data-testid="status-bar">
        <span>● SQLite</span>
        <span>3/12 节点 · 并行 2</span>
        <span>SSE 已连接</span>
        <span>耗时 4.2s</span>
      </footer>
    </div>
  )
}

function Resizer(props: {
  label: string
  className: string
  hidden?: boolean
  onDragStart: (client: number) => void
}) {
  if (props.hidden) {
    return null
  }
  return (
    <div
      role="separator"
      aria-orientation={props.className.includes('resize-v') ? 'vertical' : 'horizontal'}
      aria-label={props.label}
      className={props.className}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId)
        props.onDragStart(props.className.includes('resize-v') ? event.clientX : event.clientY)
      }}
    />
  )
}
