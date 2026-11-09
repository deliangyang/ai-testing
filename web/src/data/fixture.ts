import type { DataSource, RunEventHandler, SubscribeRunEventsOptions } from './datasource'
import { DataSourceError } from './errors'
import { DEFAULT_SCENARIO, SCENARIOS, type ScenarioFixture } from './fixtures'
import { copilotFixtureResponse } from '../copilot/fixtures'
import { MARKDOWN_IMPORT_PREVIEW } from '../import/fixtures'
import { SERIAL_AI_PENDING_DETAIL, SERIAL_AI_PENDING_VERSION } from '../pending/fixtures'
import type {
  ApiImportCommitRequest,
  ApiImportPersistSummary,
  ApiImportPreviewRequest,
  ApiImportPreviewResponse,
  CopilotChatRequest,
  CopilotChatResponse,
  DagAiRepairResponse,
  CreateRunRequest,
  CreateRunResponse,
  DagDetailDto,
  DagVersionDto,
  GateResponse,
  ListRunsQuery,
  NodeExecutionDto,
  RunDto,
} from './types'
import type { Dag } from '../types/generated'

function clone<T>(value: T): T {
  return structuredClone(value)
}

export class FixtureDataSource implements DataSource {
  private scenario: ScenarioFixture
  private pendingVersions: DagVersionDto[] = []
  private readonly liveTokens = new Set<string>()

  constructor(scenarioId: string = DEFAULT_SCENARIO) {
    if (!(scenarioId in SCENARIOS)) {
      throw new DataSourceError(`unknown fixture scenario '${scenarioId}'`)
    }
    const scenario = SCENARIOS[scenarioId]
    this.scenario = scenario
  }

  listDags(): Promise<DagVersionDto[]> {
    return Promise.resolve(clone([...this.scenario.dagVersions, ...this.pendingVersions]))
  }

  listPendingAiDags(): Promise<DagVersionDto[]> {
    if (this.scenario.id !== 'serial') {
      return Promise.resolve([])
    }
    return Promise.resolve(clone([SERIAL_AI_PENDING_VERSION, ...this.pendingVersions]))
  }

  getActiveDag(dagId: string): Promise<DagDetailDto> {
    if (dagId !== this.scenario.activeDag.id) {
      return Promise.reject(
        new DataSourceError(`DAG '${dagId}' has no active version`, { status: 404 }),
      )
    }
    return Promise.resolve(clone(this.scenario.activeDag))
  }

  getDagVersion(dagId: string, version: number): Promise<DagDetailDto> {
    if (
      dagId === SERIAL_AI_PENDING_DETAIL.id &&
      version === SERIAL_AI_PENDING_DETAIL.version &&
      this.scenario.id === 'serial'
    ) {
      return Promise.resolve(clone(SERIAL_AI_PENDING_DETAIL))
    }
    if (dagId === this.scenario.activeDag.id && version === this.scenario.activeDag.version) {
      return Promise.resolve(clone(this.scenario.activeDag))
    }
    const pending = this.pendingVersions.find((v) => v.id === dagId && v.version === version)
    if (pending !== undefined) {
      const dag = this.scenario.dags[dagId]
      return Promise.resolve({
        ...pending,
        dag: clone(dag),
      })
    }
    return Promise.reject(
      new DataSourceError(`DAG '${dagId}' version ${String(version)}`, { status: 404 }),
    )
  }

  listRuns(query?: ListRunsQuery): Promise<RunDto[]> {
    let rows = clone(this.scenario.runs)
    if (query?.dag_id) {
      rows = rows.filter((r) => r.dag_id === query.dag_id)
    }
    if (query?.status) {
      rows = rows.filter((r) => r.status === query.status)
    }
    if (query?.limit !== undefined) {
      rows = rows.slice(0, query.limit)
    }
    return Promise.resolve(rows)
  }

  getRun(runId: string): Promise<RunDto> {
    const run = this.scenario.runs.find((r) => r.id === runId)
    if (run === undefined) {
      return Promise.reject(new DataSourceError(`run '${runId}'`, { status: 404 }))
    }
    return Promise.resolve(clone(run))
  }

  async listExecutions(runId: string): Promise<NodeExecutionDto[]> {
    await this.getRun(runId)
    const rows = this.scenario.executionsByRun[runId] ?? []
    return clone(rows)
  }

  subscribeRunEvents(
    runId: string,
    onEvent: RunEventHandler,
    options?: SubscribeRunEventsOptions,
  ): () => void {
    if (!(runId in this.scenario.eventsByRun)) {
      throw new DataSourceError(`run '${runId}'`, { status: 404 })
    }
    const stored = this.scenario.eventsByRun[runId]
    const since = options?.since ?? 0
    const ac = new AbortController()
    this.liveTokens.add(runId)
    const onAbort = () => {
      ac.abort()
    }
    options?.signal?.addEventListener('abort', onAbort)

    const deliver = () => {
      for (const item of stored) {
        if (item.seq <= since || ac.signal.aborted) {
          continue
        }
        onEvent(clone(item.event), { seq: item.seq })
      }
    }
    queueMicrotask(deliver)

    return () => {
      ac.abort()
      options?.signal?.removeEventListener('abort', onAbort)
      this.liveTokens.delete(runId)
    }
  }

  copilotChat(request: CopilotChatRequest): Promise<CopilotChatResponse> {
    return Promise.resolve(copilotFixtureResponse(request))
  }

  async repairDagWithAi(dag: Dag): Promise<DagAiRepairResponse> {
    const report = await this.validateDag(dag)
    if (report.passed) {
      throw new DataSourceError('DAG already passes gate', { status: 400 })
    }
    const fixed = clone(dag)
    const pay = fixed.nodes.find((n) => n.id === 'pay' && n.type === 'http')
    if (pay && pay.type === 'http') {
      pay.path = '/orders/${create.output.body.id}/pay'
    }
    const gate = await this.validateDag(fixed)
    if (!gate.passed) {
      return { status: 'needs_manual_review', dag: fixed, gate }
    }
    const pending = await this.saveDag(fixed)
    return {
      status: 'accepted',
      dag: fixed,
      gate,
      pending: { ...pending, status: 'PENDING', created_by: 'ai' },
    }
  }

  validateDag(dag: Dag): Promise<GateResponse> {
    if (!dag.id || dag.nodes.length === 0) {
      return Promise.resolve({
        passed: false,
        issues: [
          {
            code: 'structure.empty',
            severity: 'error',
            message: 'DAG must have at least one node',
          },
        ],
      })
    }
    const issues: GateResponse['issues'] = []
    const nodeIds = new Set(dag.nodes.map((n) => n.id))
    for (const node of dag.nodes) {
      if (node.type === 'http' && !node.path.trim()) {
        issues.push({
          code: 'http.path_empty',
          severity: 'error',
          node_id: node.id,
          message: `HTTP 节点 ${node.id} 缺少 path`,
        })
      }
      if (node.type === 'http') {
        for (const match of node.path.matchAll(/\$\{([^.}]+)/g)) {
          const ref = match[1]
          if (ref && !nodeIds.has(ref)) {
            issues.push({
              code: 'template.unknown_node',
              severity: 'error',
              node_id: node.id,
              message: `节点 ${node.id} 引用了不存在的上游「${ref}」`,
            })
          }
        }
      }
      if (node.type === 'assert' && !nodeIds.has(node.target)) {
        issues.push({
          code: 'assert.missing_target',
          severity: 'error',
          node_id: node.id,
          message: `断言节点 ${node.id} 的目标不存在`,
        })
      }
    }
    const passed = !issues.some((i) => i.severity === 'error')
    return Promise.resolve({ passed, issues })
  }

  async saveDag(dag: Dag): Promise<DagVersionDto> {
    const report = await this.validateDag(dag)
    if (!report.passed) {
      throw new DataSourceError('gate rejected', { status: 422, body: report })
    }
    const nextVersion =
      Math.max(
        0,
        ...[...this.scenario.dagVersions, ...this.pendingVersions]
          .filter((v) => v.id === dag.id)
          .map((v) => v.version),
      ) + 1
    const row: DagVersionDto = {
      id: dag.id,
      version: nextVersion,
      status: 'PENDING',
      created_by: 'fixture',
      created_at: new Date().toISOString(),
    }
    this.scenario.dags[dag.id] = clone(dag)
    this.pendingVersions.push(row)
    return clone(row)
  }

  async activateDag(dagId: string, version: number): Promise<DagVersionDto> {
    await this.getDagVersion(dagId, version)
    const idx = this.scenario.dagVersions.findIndex((v) => v.id === dagId)
    if (idx >= 0) {
      this.scenario.dagVersions[idx] = {
        ...this.scenario.dagVersions[idx],
        status: 'ARCHIVED',
      }
    }
    const pendingIdx = this.pendingVersions.findIndex(
      (v) => v.id === dagId && v.version === version,
    )
    const activated: DagVersionDto = {
      id: dagId,
      version,
      status: 'ACTIVE',
      created_by: 'fixture',
      created_at: new Date().toISOString(),
    }
    if (pendingIdx >= 0) {
      this.pendingVersions.splice(pendingIdx, 1)
    }
    if (idx >= 0) {
      this.scenario.dagVersions[idx] = activated
    } else {
      this.scenario.dagVersions.push(activated)
    }
    const dag = this.scenario.dags[dagId]
    this.scenario.activeDag = { ...activated, dag: clone(dag) }
    return clone(activated)
  }

  startRun(request: CreateRunRequest): Promise<CreateRunResponse> {
    if (request.dag_id !== this.scenario.activeDag.id) {
      return Promise.reject(
        new DataSourceError(`DAG '${request.dag_id}' has no active version`, {
          status: 400,
        }),
      )
    }
    const runId = this.scenario.runs[0]?.id ?? 'run-fixture-new'
    return Promise.resolve({ run_id: runId })
  }

  cancelRun(runId: string): Promise<{ cancelled: boolean }> {
    if (!this.liveTokens.has(runId)) {
      return Promise.reject(
        new DataSourceError(`run '${runId}' is not running on this server`, { status: 404 }),
      )
    }
    return Promise.resolve({ cancelled: true })
  }

  previewApiImport(request: ApiImportPreviewRequest): Promise<ApiImportPreviewResponse> {
    const strategy = request.strategy ?? 'merge'
    return Promise.resolve(
      clone({
        ...MARKDOWN_IMPORT_PREVIEW,
        strategy,
      }),
    )
  }

  commitApiImport(request: ApiImportCommitRequest): Promise<ApiImportPersistSummary> {
    const accepted = request.items.filter((row) => row.accepted).length
    return Promise.resolve({
      added: accepted,
      updated: 0,
      unchanged: 0,
      saved_ids: request.items.filter((row) => row.accepted).map((row) => row.spec.id),
    })
  }
}
