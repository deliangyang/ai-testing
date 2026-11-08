import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react'

import { DagReportReplayView, DagRunCanvas } from '../canvas'
import { reportPreviousFor } from '../data/fixtures/reportPrevious'
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
import { loadShellLayout, saveShellLayout } from './layoutStorage'
import { mergeShellLayout, workspaceScopeKey } from './shellLayout'
import { workspacePresetById } from './workspacePresets'
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
import { GlobalSearchDialog } from './GlobalSearchDialog'
import { ShortcutHelpDialog } from './ShortcutHelpDialog'
import type { GlobalSearchItem } from './globalSearch'
import { useAppShellShortcuts } from './useAppShellShortcuts'
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
  const [searchOpen, setSearchOpen] = useState(false)
  const [helpOpen, setHelpOpen] = useState(false)
  const [fitViewToken, setFitViewToken] = useState(0)

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

  const layoutScopeKey = activeTab
    ? workspaceScopeKey(activeTab.kind, activeTab.refId)
    : workspaceScopeKey(INITIAL_WORKSPACE_TAB.kind, INITIAL_WORKSPACE_TAB.refId)

  useEffect(() => {
    const stored = loadShellLayout(layoutScopeKey)
    if (!stored) {
      return
    }
    setNavCollapsed(stored.nav_collapsed)
    setRightWidth(clampRightPanelWidth(stored.right_width_px))
    setDrawerHeight(clampBottomDrawerHeight(stored.drawer_height_px))
    setDrawerCollapsed(stored.drawer_collapsed)
  }, [layoutScopeKey])

  useEffect(() => {
    saveShellLayout(layoutScopeKey, {
      nav_collapsed: navCollapsed,
      right_width_px: rightWidth,
      drawer_height_px: drawerHeight,
      drawer_collapsed: drawerCollapsed,
    })
  }, [layoutScopeKey, navCollapsed, rightWidth, drawerHeight, drawerCollapsed])

  const runBlocked = runControl.phase === 'running' || runControl.phase === 'cancelling'

  const openSearch = useCallback(() => {
    setSearchOpen(true)
  }, [])

  const openHelp = useCallback(() => {
    setHelpOpen(true)
  }, [])

  const handleFitCanvas = useCallback(() => {
    setFitViewToken((n) => n + 1)
  }, [])

  const handleSearchPick = useCallback(
    (item: GlobalSearchItem) => {
      if (item.kind === 'scenario') {
        openScenarioTab(item.refId)
        return
      }
      openReportTab(item.refId)
    },
    [openReportTab, openScenarioTab],
  )

  useAppShellShortcuts({
    openSearch,
    openHelp,
    onRun: handleRun,
    onRerunFailed: handleRerunFailed,
    onModeChange: (mode) => {
      dispatchRunControl({ type: 'set_mode', mode })
    },
    onFitCanvas: handleFitCanvas,
    canRerunFailed: runControl.canRerunFailed,
    runBlocked,
    anyDialogOpen: searchOpen || helpOpen,
  })

  const handleWorkspacePreset = useCallback((presetId: string) => {
    const preset = workspacePresetById(presetId)
    if (!preset) {
      return
    }
    const layout = mergeShellLayout(preset.layout)
    setNavCollapsed(layout.nav_collapsed)
    setRightWidth(clampRightPanelWidth(layout.right_width_px))
    setDrawerHeight(clampBottomDrawerHeight(layout.drawer_height_px))
    setDrawerCollapsed(layout.drawer_collapsed)
  }, [])

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
      data-layout-scope={layoutScopeKey}
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
          onWorkspacePreset={handleWorkspacePreset}
          onOpenSearch={openSearch}
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
                {replayMode && activeWorkspace.run ? (
                  <DagReportReplayView
                    dag={failureFocus.dag}
                    events={failureFocus.events}
                    run={activeWorkspace.run}
                    previous={
                      activeTab ? reportPreviousFor(activeTab.refId) : undefined
                    }
                    focusNodeId={failureFocus.failedNodeId}
                    sourceRunId={sourceRunId}
                    partialRerunNode={partialRerunNode}
                    onRerunFromNode={handleRerunFromNode}
                    fitViewToken={fitViewToken}
                  />
                ) : (
                  <DagRunCanvas
                    dag={failureFocus.dag}
                    events={failureFocus.events}
                    focusNodeId={failureFocus.failedNodeId}
                    sourceRunId={sourceRunId}
                    partialRerunNode={partialRerunNode}
                    onRerunFromNode={handleRerunFromNode}
                    fitViewToken={fitViewToken}
                  />
                )}
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

      <GlobalSearchDialog
        open={searchOpen}
        catalog={FIXTURE_NAV_CATALOG}
        onClose={() => {
          setSearchOpen(false)
        }}
        onPick={handleSearchPick}
      />
      <ShortcutHelpDialog
        open={helpOpen}
        onClose={() => {
          setHelpOpen(false)
        }}
      />
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
