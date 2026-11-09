/** API DTO shapes aligned with `crates/server/src/dto.rs` (not yet schema-generated). */

import type { Dag } from '../types/generated'

export type Timestamp = string

export interface DagVersionDto {
  id: string
  version: number
  status: string
  created_by: string
  created_at: Timestamp
}

export interface DagDetailDto extends DagVersionDto {
  dag: Dag
}

export interface RunDto {
  id: string
  dag_id: string
  dag_version: number
  env_name: string
  mode: string
  status: string
  started_at: Timestamp | null
  finished_at: Timestamp | null
}

export interface FailureDto {
  category: string
  error_code: string
}

export interface NodeExecutionDto {
  run_id: string
  node_id: string
  attempt: number
  status: string
  input_resolved: unknown
  output: unknown
  poll_trace: unknown
  failure: FailureDto | null
  started_at: Timestamp | null
  finished_at: Timestamp | null
}

export interface ListRunsQuery {
  dag_id?: string
  status?: string
  limit?: number
}

export interface GateResponse {
  passed: boolean
  issues: GateIssue[]
}

export interface GateIssue {
  code: string
  severity: 'error' | 'warn'
  node_id?: string
  json_path?: string
  message: string
}

export interface CreateRunRequest {
  dag_id: string
  env: string
  parallel?: number
}

export interface CreateRunResponse {
  run_id: string
}

export type ApiImportFormat = 'openapi' | 'postman' | 'curl' | 'har' | 'md'

export type ApiImportMergeStrategy = 'merge' | 'overwrite' | 'skip'

export type ApiImportMergeAction = 'add' | 'update' | 'unchanged' | 'skip'

export interface ApiSpecDto {
  id: string
  name: string
  method: string
  path: string
  params?: unknown[]
  response_schema?: unknown
  auth_scheme_ref?: string
}

export interface ApiImportPreviewItem {
  spec: ApiSpecDto
  confidence: number
  source_lines: number[]
  merge_action: ApiImportMergeAction
}

export interface ApiImportPreviewSummary {
  add: number
  update: number
  unchanged: number
  skip: number
}

export interface ApiImportPreviewResponse {
  strategy: ApiImportMergeStrategy
  items: ApiImportPreviewItem[]
  summary: ApiImportPreviewSummary
}

export interface ApiImportPreviewRequest {
  format: ApiImportFormat
  content: string
  strategy?: ApiImportMergeStrategy
}

export interface ApiImportCommitItem {
  spec: ApiSpecDto
  accepted: boolean
}

export interface ApiImportCommitRequest {
  strategy: ApiImportMergeStrategy
  items: ApiImportCommitItem[]
}

export interface ApiImportPersistSummary {
  added: number
  updated: number
  unchanged: number
  saved_ids: string[]
}

export type NodeStatus =
  'PENDING' | 'READY' | 'RUNNING' | 'POLLING' | 'COMPLETED' | 'FAILED' | 'SKIPPED'

export type LogLevel = 'info' | 'warn' | 'error'

export type RunTerminalStatus = 'COMPLETED' | 'FAILED' | 'CANCELLED'

export type EngineEvent =
  | { type: 'node_state_changed'; node: string; status: NodeStatus }
  | { type: 'poll_iteration'; node: string; attempt: number; entry: unknown }
  | { type: 'log'; node?: string; level: LogLevel; message: string }
  | { type: 'run_finished'; status: RunTerminalStatus }

export interface StoredRunEvent {
  seq: number
  event: EngineEvent
}
