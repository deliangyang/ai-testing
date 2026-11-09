//! AI 副驾对话（fixture / 可选 Cassette 回放，develop.md §5.2 P2-24）。

use axum::Json;
use model::nodes::Condition;
use model::{Edge, Node};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{ApiError, ApiResult};

const FIXTURE_TRIGGER: &str = "补健康检查";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CopilotAction {
    InsertCanvas {
        node: Box<Node>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        edges: Vec<Edge>,
    },
    FocusNode {
        node_id: String,
    },
    AdoptAssertion {
        node_id: String,
        assertions: Vec<Condition>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<CopilotAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotChatContext {
    pub dag_id: String,
    #[serde(default)]
    pub dag_version: Option<u32>,
    #[serde(default)]
    pub selected_node_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CopilotChatRequest {
    pub context: CopilotChatContext,
    pub message: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub history: Vec<CopilotChatMessage>,
}

#[derive(Debug, Serialize)]
pub struct CopilotChatResponse {
    pub message: CopilotChatMessage,
}

fn fixture_response(request: &CopilotChatRequest) -> ApiResult<CopilotChatResponse> {
    let trimmed = request.message.trim();
    if trimmed != FIXTURE_TRIGGER {
        return Ok(CopilotChatResponse {
            message: CopilotChatMessage {
                role: "assistant".into(),
                content: "请说明要在当前 DAG 上做什么；试用话术「补健康检查」可查看示例工件绑定。"
                    .into(),
                node_refs: vec![],
                actions: vec![],
            },
        });
    }
    let focus = request.context.selected_node_id.as_deref().unwrap_or("pay");
    let health: Box<Node> = Box::new(
        serde_json::from_value(json!({
            "id": "health",
            "type": "http",
            "method": "GET",
            "path": "/health"
        }))
        .map_err(|e| ApiError::Internal(e.to_string()))?,
    );
    let paid: Condition = serde_json::from_value(json!({
        "path": "$.body.paid",
        "op": "eq",
        "expected": true
    }))
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(CopilotChatResponse {
        message: CopilotChatMessage {
            role: "assistant".into(),
            content: format!(
                "可在 create 后插入 health 探活节点；当前上下文节点 {focus} 可定位；并为 check 追加 paid 断言。"
            ),
            node_refs: vec![
                "create".into(),
                "health".into(),
                focus.to_string(),
                "check".into(),
            ],
            actions: vec![
                CopilotAction::InsertCanvas {
                    node: health,
                    edges: vec![Edge {
                        from: "create".into(),
                        to: "health".into(),
                    }],
                },
                CopilotAction::FocusNode {
                    node_id: "pay".into(),
                },
                CopilotAction::AdoptAssertion {
                    node_id: "check".into(),
                    assertions: vec![paid],
                },
            ],
        },
    })
}

/// 当 `AITEST_CASSETTE_DIR` 已设置时可扩展 Cassette 回放；当前使用确定性 fixture。
pub async fn copilot_chat(
    Json(request): Json<CopilotChatRequest>,
) -> ApiResult<Json<CopilotChatResponse>> {
    if request.context.dag_id.trim().is_empty() {
        return Err(ApiError::BadRequest("dag_id required".into()));
    }
    let _ = &request.history;
    let _cassette_hint = std::env::var("AITEST_CASSETTE_DIR").ok();
    Ok(Json(fixture_response(&request)?))
}
