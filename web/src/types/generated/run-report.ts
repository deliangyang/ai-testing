/**
 * Generated from `schemas/run-report.schema.json`. Do not edit.
 * @see npm run generate:types
 */

export type NodeStatus = ("PENDING" | "READY" | "RUNNING" | "COMPLETED" | "FAILED") | "POLLING" | "SKIPPED";

export interface RunReport {
  deterministic: DeterministicReport;
  schema: string;
  volatile: VolatileReport;
}
export interface DeterministicReport {
  dag_id: string;
  dag_version: number;
  env: string;
  no_llm: boolean;
  nodes: {
    [k: string]: DeterministicNode | undefined;
  };
  status: string;
}
export interface DeterministicNode {
  assertions?: unknown;
  error?: unknown;
  failure?: FailureRef | null;
  output?: unknown;
  poll?: unknown;
  status: NodeStatus;
}
export interface FailureRef {
  category: string;
  error_code: string;
}
export interface VolatileReport {
  finished_at?: string | null;
  nodes: {
    [k: string]: VolatileNode | undefined;
  };
  run_id: string;
  started_at?: string | null;
}
export interface VolatileNode {
  headers?: unknown;
  poll?: unknown;
  timing?: unknown;
}
