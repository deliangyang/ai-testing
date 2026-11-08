use common::clock::Timestamp;
use gate::Report;
use model::NodeStatus;
use serde::Serialize;
use serde_json::Value;
use store::{
    DagRecord, DagStatus, DagVersionInfo, Failure, NodeExecution, RunMode, RunRecord, RunStatus,
    UiLayoutRecord, WorkspacePresetRecord,
};

#[derive(Debug, Clone, Serialize)]
pub struct GateResponse {
    pub passed: bool,
    pub issues: Vec<gate::Issue>,
}

impl From<Report> for GateResponse {
    fn from(report: Report) -> Self {
        Self {
            passed: report.passed(),
            issues: report.issues,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DagVersionDto {
    pub id: String,
    pub version: u32,
    pub status: String,
    pub created_by: String,
    pub created_at: Timestamp,
}

impl From<DagVersionInfo> for DagVersionDto {
    fn from(v: DagVersionInfo) -> Self {
        Self {
            id: v.id,
            version: v.version,
            status: v.status.to_string(),
            created_by: v.created_by,
            created_at: v.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DagDetailDto {
    #[serde(flatten)]
    pub info: DagVersionDto,
    pub dag: Value,
}

impl DagDetailDto {
    pub fn from_record(record: DagRecord) -> Result<Self, serde_json::Error> {
        Ok(Self {
            info: DagVersionDto::from(record.info),
            dag: serde_json::to_value(record.dag)?,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunDto {
    pub id: String,
    pub dag_id: String,
    pub dag_version: u32,
    pub env_name: String,
    pub mode: String,
    pub status: String,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
}

impl From<RunRecord> for RunDto {
    fn from(r: RunRecord) -> Self {
        Self {
            id: r.id,
            dag_id: r.dag_id,
            dag_version: r.dag_version,
            env_name: r.env_name,
            mode: r.mode.as_str().to_owned(),
            status: r.status.as_str().to_owned(),
            started_at: r.started_at,
            finished_at: r.finished_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FailureDto {
    pub category: String,
    pub error_code: String,
}

impl From<Failure> for FailureDto {
    fn from(f: Failure) -> Self {
        Self {
            category: f.category,
            error_code: f.error_code,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeExecutionDto {
    pub run_id: String,
    pub node_id: String,
    pub attempt: u32,
    pub status: String,
    pub input_resolved: Option<Value>,
    pub output: Option<Value>,
    pub poll_trace: Option<Value>,
    pub failure: Option<FailureDto>,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
}

impl From<NodeExecution> for NodeExecutionDto {
    fn from(n: NodeExecution) -> Self {
        Self {
            run_id: n.key.run_id,
            node_id: n.key.node_id,
            attempt: n.key.attempt,
            status: format_node_status(n.status),
            input_resolved: n.input_resolved,
            output: n.output,
            poll_trace: n.poll_trace,
            failure: n.failure.map(Into::into),
            started_at: n.started_at,
            finished_at: n.finished_at,
        }
    }
}

fn format_node_status(s: NodeStatus) -> String {
    s.as_str().to_owned()
}

#[allow(dead_code)]
pub fn dag_status_label(s: DagStatus) -> &'static str {
    s.as_str()
}

#[allow(dead_code)]
pub fn run_status_label(s: RunStatus) -> &'static str {
    s.as_str()
}

#[allow(dead_code)]
pub fn run_mode_label(s: RunMode) -> &'static str {
    s.as_str()
}

#[derive(Debug, Clone, Serialize)]
pub struct UiLayoutDto {
    pub scope_key: String,
    pub layout: Value,
    pub updated_at: Timestamp,
}

impl From<UiLayoutRecord> for UiLayoutDto {
    fn from(record: UiLayoutRecord) -> Self {
        Self {
            scope_key: record.scope_key,
            layout: record.layout,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspacePresetDto {
    pub id: String,
    pub name: String,
    pub layout: Value,
    pub updated_at: Timestamp,
}

impl From<WorkspacePresetRecord> for WorkspacePresetDto {
    fn from(record: WorkspacePresetRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            layout: record.layout,
            updated_at: record.updated_at,
        }
    }
}
