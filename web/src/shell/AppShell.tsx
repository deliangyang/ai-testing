import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { DagRunCanvas } from '../canvas'
import {
  ASYNC_POLL_FAIL_RUN,
  ASYNC_POLL_FAIL_SCENARIO,
} from '../data/fixtures/asyncPollFail'
import { BottomDrawer } from '../drawer/BottomDrawer'
import '../drawer/drawer.css'
import { watchVariablesFromExecutions } from '../drawer/watchVariables'
import { useFailureFocus } from '../focus'
import { FIXTURE_NAV_CATALOG, LeftNav } from '../nav'
import '../nav/nav.css'
import { ExecutionDetailPanel } from '../panels'
import { clampBottomDrawerHeight, clampRightPanelWidth, leftNavWidthPx } from './layout'
import { StatusBar } from './StatusBar'
import { buildStatusBarModel } from './statusBarModel'
import { shellTokens } from './tokens'
import './shell.css'

export function AppShell() {
  const failureFocus = useFailureFocus(ASYNC_POLL_FAIL_SCENARIO)
  const statusBar = useMemo(
    () =>
      buildStatusBarModel({
        dag: failureFocus.dag,
        events: failureFocus.events,
        run: ASYNC_POLL_FAIL_RUN,
        connections: { sqliteConnected: true, sseConnected: true },
      }),
    [failureFocus.dag, failureFocus.events],
  )
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
          <LeftNav
            catalog={FIXTURE_NAV_CATALOG}
            collapsed={navCollapsed}
            onToggleCollapse={() => {
              setNavCollapsed((v) => !v)
            }}
          />
        </nav>

        <div className="app-shell__center-col">
          <div className="app-shell__center-main">
            <section className="app-shell__center" aria-label="工作区">
              <div className="app-shell__tabs" role="tablist">
                <span role="tab" aria-selected>
                  场景 async-poll
                </span>
              </div>
              <main className="app-shell__canvas" data-testid="center-canvas">
                <DagRunCanvas
                  dag={failureFocus.dag}
                  events={failureFocus.events}
                  focusNodeId={failureFocus.failedNodeId}
                />
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
                nodeId={failureFocus.failedNodeId ?? ''}
                execution={failureFocus.execution}
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
            {!drawerCollapsed ? (
              <div className="app-shell__drawer-body">
                <BottomDrawer
                  logs={failureFocus.allLogs}
                  pollRows={failureFocus.pollRows}
                  variables={watchVariablesFromExecutions(failureFocus.executions)}
                  initialNodeFilter={failureFocus.failedNodeId}
                />
              </div>
            ) : null}
          </section>
        </div>
      </div>

      <StatusBar model={statusBar} />
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
