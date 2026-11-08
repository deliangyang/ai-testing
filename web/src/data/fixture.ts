import type { DataSource, RunEventHandler, SubscribeRunEventsOptions } from './datasource'
import { DataSourceError } from './errors'
import { DEFAULT_SCENARIO, SCENARIOS, type ScenarioFixture } from './fixtures'
import type {
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

  getActiveDag(dagId: string): Promise<DagDetailDto> {
    if (dagId !== this.scenario.activeDag.id) {
      return Promise.reject(
        new DataSourceError(`DAG '${dagId}' has no active version`, { status: 404 }),
      )
    }
    return Promise.resolve(clone(this.scenario.activeDag))
  }

  getDagVersion(dagId: string, version: number): Promise<DagDetailDto> {
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
    return Promise.resolve({ passed: true, issues: [] })
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
}
