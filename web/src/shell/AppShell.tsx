import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react'

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
import { resolveWorkspaceForTab } from './resolveWorkspace'
import { StatusBar } from './StatusBar'
import { buildStatusBarModel } from './statusBarModel'
import { shellTokens } from './tokens'
import { TopBar } from './TopBar'
import {
  initialRunControlState,
  runControlReducer,
  type RunControlAction,
} from './runControl'
import { WorkspaceTabBar } from './WorkspaceTabBar'
import {
  activeTabAfterClose,
  closeWorkspaceTab,
  openWorkspaceTab,
  reportTabKey,
  scenarioTabKey,
  tabLabelForReport,
  tabLabelForScenario,
  type WorkspaceTab,
} from './workspaceTabs'
import './shell.css'

const INITIAL_WORKSPACE_TAB: WorkspaceTab = {
  key: scenarioTabKey('async-poll'),
  kind: 'scenario',
  refId: 'async-poll',
  label: tabLabelForScenario('async-poll'),
}

export function AppShell() {
  const [workspaceTabs, setWorkspaceTabs] = useState<WorkspaceTab[]>([INITIAL_WORKSPACE_TAB])
  const [activeTabKey, setActiveTabKey] = useState<string>(INITIAL_WORKSPACE_TAB.key)

  const activeWorkspace = useMemo(() => {
    const active = workspaceTabs.find((tab) => tab.key === activeTabKey)
    if (!active) {
      return ASYNC_POLL_FAIL_SCENARIO
    }
    return resolveWorkspaceForTab(active, FIXTURE_NAV_CATALOG) ?? ASYNC_POLL_FAIL_SCENARIO
  }, [workspaceTabs, activeTabKey])

  const [runControl, dispatchRunControl] = useReducer(runControlReducer, initialRunControlState)
  const [partialRerunNode, setPartialRerunNode] = useState<string | null>(null)

  const activeTab = useMemo(
    () => workspaceTabs.find((tab) => tab.key === activeTabKey),
    [workspaceTabs, activeTabKey],
  )
  const replayMode = activeTab?.kind === 'report'

  useEffect(() => {
    const failed = activeWorkspace.run?.status === 'FAILED'
    dispatchRunControl({ type: 'set_can_rerun_failed', value: failed })
  }, [activeWorkspace])

  const failureFocus = useFailureFocus(activeWorkspace)
  const statusBar = useMemo(
    () =>
      buildStatusBarModel({
        dag: failureFocus.dag,
        events: failureFocus.events,
        run: {
          ...(activeWorkspace.run ?? ASYNC_POLL_FAIL_RUN),
          env_name: runControl.envName,
        },
        connections: { sqliteConnected: true, sseConnected: true },
      }),
    [failureFocus.dag, failureFocus.events, activeWorkspace.run, runControl.envName],
  )

  const dispatchRun = useCallback((action: RunControlAction) => {
    dispatchRunControl(action)
  }, [])

  const handleRun = useCallback(() => {
    dispatchRun({ type: 'start_run' })
  }, [dispatchRun])

  const handleStop = useCallback(() => {
    dispatchRun({ type: 'stop_run' })
    queueMicrotask(() => {
      dispatchRun({ type: 'finish_run', failed: false })
    })
  }, [dispatchRun])

  const handleRerunFailed = useCallback(() => {
    dispatchRun({ type: 'start_run' })
  }, [dispatchRun])

  const handleRerunFromNode = useCallback(
    (nodeId: string) => {
      setPartialRerunNode(nodeId)
      dispatchRun({ type: 'start_run' })
    },
    [dispatchRun],
  )

  const sourceRunId = activeWorkspace.run?.id

  const openScenarioTab = useCallback((scenarioId: string) => {
    const title =
      FIXTURE_NAV_CATALOG.scenarios.find((item) => item.id === scenarioId)?.title ?? scenarioId
    const tab: WorkspaceTab = {
      key: scenarioTabKey(scenarioId),
      kind: 'scenario',
      refId: scenarioId,
      label: tabLabelForScenario(title),
    }
    setWorkspaceTabs((prev) => openWorkspaceTab(prev, tab))
    setActiveTabKey(tab.key)
  }, [])

  const openReportTab = useCallback((reportId: string) => {
    const title = FIXTURE_NAV_CATALOG.reports.find((item) => item.id === reportId)?.title ?? reportId
    const tab: WorkspaceTab = {
      key: reportTabKey(reportId),
      kind: 'report',
      refId: reportId,
      label: tabLabelForReport(title),
    }
    setWorkspaceTabs((prev) => openWorkspaceTab(prev, tab))
    setActiveTabKey(tab.key)
  }, [])

  const closeTab = useCallback((key: string) => {
    setWorkspaceTabs((prev) => {
      const next = closeWorkspaceTab(prev, key)
      setActiveTabKey((current) => activeTabAfterClose(next, key, current) ?? INITIAL_WORKSPACE_TAB.key)
      return next.length > 0 ? next : [INITIAL_WORKSPACE_TAB]
    })
  }, [])

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
        <TopBar
          runControl={runControl}
          onEnvChange={(envName) => {
            dispatchRunControl({ type: 'set_env', envName })
          }}
          onRun={handleRun}
          onStop={handleStop}
          onRerunFailed={handleRerunFailed}
          onModeChange={(mode) => {
            dispatchRunControl({ type: 'set_mode', mode })
          }}
        />
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
            onOpenScenario={openScenarioTab}
            onOpenReport={openReportTab}
          />
        </nav>

        <div className="app-shell__center-col">
          <div className="app-shell__center-main">
            <section className="app-shell__center" aria-label="工作区">
              <WorkspaceTabBar
                tabs={workspaceTabs}
                activeKey={activeTabKey}
                onSelect={setActiveTabKey}
                onClose={closeTab}
              />
              <main className="app-shell__canvas" data-testid="center-canvas">
                <DagRunCanvas
                  dag={failureFocus.dag}
                  events={failureFocus.events}
                  focusNodeId={failureFocus.failedNodeId}
                  replayMode={replayMode}
                  sourceRunId={sourceRunId}
                  partialRerunNode={partialRerunNode}
                  onRerunFromNode={handleRerunFromNode}
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
