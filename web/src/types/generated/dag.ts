/**
 * Generated from `schemas/dag.schema.json`. Do not edit.
 * @see npm run generate:types
 */

export type Node =
  | {
      /**
       * Active `api_specs` row to resolve response shape when set; overrides inline `response_schema` for gate checks.
       */
      api_spec_id?: string | null;
      body?: unknown;
      headers?: {
        [k: string]: string | undefined;
      };
      id: string;
      method: HttpMethod;
      name?: string;
      path: string;
      query?: {
        [k: string]: string | undefined;
      };
      /**
       * JSON Schema of the response body; used by the gate to check `${node.output.body...}`.
       */
      response_schema?: {
        [k: string]: unknown | undefined;
      };
      timeout_ms?: number | null;
      type: "http";
    }
  | {
      backoff_ms: number[];
      condition: Condition;
      id: string;
      max_attempts?: number | null;
      name?: string;
      request: HttpRequest;
      response_schema?: unknown;
      timeout_ms: number;
      type: "poll";
    }
  | {
      assertions: Condition[];
      id: string;
      name?: string;
      /**
       * Id of the node whose output is checked; must be an ancestor.
       */
      target: string;
      type: "assert";
    }
  | {
      /**
       * Name of a connection asset bound to the current environment.
       */
      connection: string;
      id: string;
      name?: string;
      /**
       * Positional parameters; string leaves are templates.
       */
      params?: unknown[];
      response_schema?: unknown;
      result?: QueryResultMode;
      /**
       * Uses the driver's positional placeholders; values never get interpolated into SQL text.
       */
      sql: string;
      type: "db_query";
    }
  | {
      dag?: DagRef | null;
      id: string;
      /**
       * Input name -> value; templates evaluated in the parent scope.
       */
      inputs?: {
        [k: string]: unknown | undefined;
      };
      name?: string;
      /**
       * Output name -> template evaluated in the child scope, e.g. `${login.output.body.token}`.
       */
      outputs?: {
        [k: string]: string | undefined;
      };
      skill?: SkillRef | null;
      type: "subworkflow";
    }
  | {
      id: string;
      name?: string;
      /**
       * Executed in order; a failing request does not stop the remaining ones.
       */
      requests?: HttpRequest[];
      type: "cleanup";
    };
export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";
export type Operator =
  "eq" | "ne" | "gt" | "gte" | "lt" | "lte" | "contains" | "exists" | "not_exists" | "regex" | "len_eq" | "type_is";
export type QueryResultMode = "rows" | "first_row" | "scalar";

export interface Dag {
  edges?: Edge[];
  id: string;
  meta?: {
    [k: string]: unknown | undefined;
  };
  name?: string;
  nodes: Node[];
  /**
   * Assigned by the store on save; absent in hand-written files.
   */
  version?: number | null;
}
export interface Edge {
  from: string;
  to: string;
}
/**
 * A mechanically decidable check: JSONPath into a node output (`{status, headers, body}`),
 * an operator and the expected value.
 */
export interface Condition {
  /**
   * An explicit `null` is a value to compare against, distinct from an absent key.
   */
  expected?: {
    [k: string]: unknown | undefined;
  };
  op: Operator;
  /**
   * e.g. `$.body.status`
   */
  path: string;
}
/**
 * A request description. String leaves of `path`, `query`, `headers` and `body` are templates.
 */
export interface HttpRequest {
  body?: unknown;
  headers?: {
    [k: string]: string | undefined;
  };
  method: HttpMethod;
  /**
   * Relative to the environment `base_url` unless it starts with `http://` or `https://`.
   */
  path: string;
  query?: {
    [k: string]: string | undefined;
  };
  timeout_ms?: number | null;
}
/**
 * Pin to a stored DAG. `version: None` resolves to the active version at run time.
 */
export interface DagRef {
  id: string;
  version?: number | null;
}
/**
 * Pin to a stored Skill. `version_req: None` resolves to the active Skill at run time;
 * otherwise the highest matching manifest semver among activated revisions wins.
 */
export interface SkillRef {
  id: string;
  version_req?: string | null;
}
