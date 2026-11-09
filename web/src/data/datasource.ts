import type {
  ApiImportCommitRequest,
  ApiImportPersistSummary,
  ApiImportPreviewRequest,
  ApiImportPreviewResponse,
  CreateRunRequest,
  CreateRunResponse,
  DagAiRepairResponse,
  DagDetailDto,
  DagVersionDto,
  EngineEvent,
  GateResponse,
  ListRunsQuery,
  NodeExecutionDto,
  RunDto,
} from './types'
import type { Dag } from '../types/generated'

export interface RunEventMeta {
  seq: number
}

export type RunEventHandler = (event: EngineEvent, meta: RunEventMeta) => void

export interface SubscribeRunEventsOptions {
  /** Replay events after this sequence (SSE `since` / `Last-Event-ID`). */
  since?: number
  signal?: AbortSignal
}

/** UI data access; components must not call `fetch` directly. */
export interface DataSource {
  listDags(): Promise<DagVersionDto[]>
  listPendingAiDags(): Promise<DagVersionDto[]>
  getActiveDag(dagId: string): Promise<DagDetailDto>
  getDagVersion(dagId: string, version: number): Promise<DagDetailDto>

  listRuns(query?: ListRunsQuery): Promise<RunDto[]>
  getRun(runId: string): Promise<RunDto>
  listExecutions(runId: string): Promise<NodeExecutionDto[]>

  subscribeRunEvents(
    runId: string,
    onEvent: RunEventHandler,
    options?: SubscribeRunEventsOptions,
  ): () => void

  validateDag(dag: Dag): Promise<GateResponse>
  repairDagWithAi(dag: Dag): Promise<DagAiRepairResponse>
  saveDag(dag: Dag): Promise<DagVersionDto>
  activateDag(dagId: string, version: number): Promise<DagVersionDto>
  startRun(request: CreateRunRequest): Promise<CreateRunResponse>
  cancelRun(runId: string): Promise<{ cancelled: boolean }>
  previewApiImport(request: ApiImportPreviewRequest): Promise<ApiImportPreviewResponse>
  commitApiImport(request: ApiImportCommitRequest): Promise<ApiImportPersistSummary>
}
