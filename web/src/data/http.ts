import type { DataSource, RunEventHandler, SubscribeRunEventsOptions } from './datasource'
import { DataSourceError } from './errors'
import type {
  ApiImportCommitRequest,
  ApiImportPersistSummary,
  ApiImportPreviewRequest,
  ApiImportPreviewResponse,
  CreateRunRequest,
  CreateRunResponse,
  DagDetailDto,
  DagVersionDto,
  EngineEvent,
  GateResponse,
  ListRunsQuery,
  NodeExecutionDto,
  RunDto,
} from './types'
import type { Dag } from '../types/generated'

export interface HttpDataSourceDeps {
  fetch?: typeof fetch
  EventSource?: typeof EventSource
}

const SSE_EVENT_NAMES: Record<EngineEvent['type'], string> = {
  node_state_changed: 'node_state_changed',
  poll_iteration: 'poll_iteration',
  log: 'log',
  run_finished: 'run_finished',
}

export class HttpDataSource implements DataSource {
  private baseUrl: string
  private deps: HttpDataSourceDeps

  constructor(baseUrl: string, deps: HttpDataSourceDeps = {}) {
    this.baseUrl = baseUrl
    this.deps = deps
  }

  private url(path: string): string {
    return `${this.baseUrl.replace(/\/$/, '')}${path}`
  }

  private get fetchFn(): typeof fetch {
    return this.deps.fetch ?? globalThis.fetch.bind(globalThis)
  }

  private get eventSourceCtor(): typeof EventSource {
    return this.deps.EventSource ?? globalThis.EventSource
  }

  private async parseJson<T>(response: Response): Promise<T> {
    if (!response.ok) {
      let body: unknown
      try {
        body = await response.json()
      } catch {
        body = undefined
      }
      let message = response.statusText
      if (
        typeof body === 'object' &&
        body !== null &&
        'error' in body &&
        typeof body.error === 'string'
      ) {
        message = body.error
      }
      throw new DataSourceError(message, { status: response.status, body })
    }
    return response.json() as Promise<T>
  }

  async listDags(): Promise<DagVersionDto[]> {
    const response = await this.fetchFn(this.url('/api/dags'))
    return this.parseJson(response)
  }

  async listPendingAiDags(): Promise<DagVersionDto[]> {
    const response = await this.fetchFn(this.url('/api/dags/pending'))
    return this.parseJson(response)
  }

  async getActiveDag(dagId: string): Promise<DagDetailDto> {
    const response = await this.fetchFn(this.url(`/api/dags/${encodeURIComponent(dagId)}/active`))
    return this.parseJson(response)
  }

  async getDagVersion(dagId: string, version: number): Promise<DagDetailDto> {
    const response = await this.fetchFn(
      this.url(`/api/dags/${encodeURIComponent(dagId)}/versions/${String(version)}`),
    )
    return this.parseJson(response)
  }

  async listRuns(query?: ListRunsQuery): Promise<RunDto[]> {
    const params = new URLSearchParams()
    if (query?.dag_id) {
      params.set('dag_id', query.dag_id)
    }
    if (query?.status) {
      params.set('status', query.status)
    }
    if (query?.limit !== undefined) {
      params.set('limit', String(query.limit))
    }
    const qs = params.toString()
    const response = await this.fetchFn(this.url(`/api/runs${qs ? `?${qs}` : ''}`))
    return this.parseJson(response)
  }

  async getRun(runId: string): Promise<RunDto> {
    const response = await this.fetchFn(this.url(`/api/runs/${encodeURIComponent(runId)}`))
    return this.parseJson(response)
  }

  async listExecutions(runId: string): Promise<NodeExecutionDto[]> {
    const response = await this.fetchFn(
      this.url(`/api/runs/${encodeURIComponent(runId)}/executions`),
    )
    return this.parseJson(response)
  }

  subscribeRunEvents(
    runId: string,
    onEvent: RunEventHandler,
    options?: SubscribeRunEventsOptions,
  ): () => void {
    const since = options?.since ?? 0
    const streamUrl = this.url(
      `/api/runs/${encodeURIComponent(runId)}/events?since=${encodeURIComponent(String(since))}`,
    )
    const Es = this.eventSourceCtor
    if (typeof Es !== 'function') {
      throw new DataSourceError('EventSource is not available')
    }
    const source = new Es(streamUrl)

    const onSse = () => (message: MessageEvent<string>) => {
      try {
        const event = JSON.parse(message.data) as EngineEvent
        const seq = Number(message.lastEventId)
        if (!Number.isFinite(seq)) {
          return
        }
        onEvent(event, { seq })
        if (event.type === 'run_finished') {
          source.close()
        }
      } catch {
        // ignore malformed chunks in the UI stream
      }
    }

    for (const name of Object.values(SSE_EVENT_NAMES)) {
      source.addEventListener(name, onSse() as EventListener)
    }

    const abort = () => {
      source.close()
    }
    options?.signal?.addEventListener('abort', abort)

    source.onerror = () => {
      source.close()
    }

    return () => {
      options?.signal?.removeEventListener('abort', abort)
      source.close()
    }
  }

  async validateDag(dag: Dag): Promise<GateResponse> {
    const response = await this.fetchFn(this.url('/api/dags/validate'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(dag),
    })
    return this.parseJson(response)
  }

  async saveDag(dag: Dag): Promise<DagVersionDto> {
    const response = await this.fetchFn(this.url('/api/dags'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(dag),
    })
    if (response.status === 422) {
      const body = (await response.json()) as GateResponse
      throw new DataSourceError('gate rejected', { status: 422, body })
    }
    return this.parseJson(response)
  }

  async activateDag(dagId: string, version: number): Promise<DagVersionDto> {
    const response = await this.fetchFn(
      this.url(`/api/dags/${encodeURIComponent(dagId)}/versions/${String(version)}/activate`),
      { method: 'POST' },
    )
    if (response.status === 422) {
      const body = (await response.json()) as GateResponse
      throw new DataSourceError('gate rejected', { status: 422, body })
    }
    return this.parseJson(response)
  }

  async startRun(request: CreateRunRequest): Promise<CreateRunResponse> {
    const response = await this.fetchFn(this.url('/api/runs'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    })
    return this.parseJson(response)
  }

  async cancelRun(runId: string): Promise<{ cancelled: boolean }> {
    const response = await this.fetchFn(this.url(`/api/runs/${encodeURIComponent(runId)}/cancel`), {
      method: 'POST',
    })
    return this.parseJson(response)
  }

  async previewApiImport(request: ApiImportPreviewRequest): Promise<ApiImportPreviewResponse> {
    const response = await this.fetchFn(this.url('/api/api-specs/import/preview'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    })
    return this.parseJson(response)
  }

  async commitApiImport(request: ApiImportCommitRequest): Promise<ApiImportPersistSummary> {
    const response = await this.fetchFn(this.url('/api/api-specs/import/commit'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    })
    return this.parseJson(response)
  }
}
