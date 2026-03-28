use axum::{
    extract::State as AxumState,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::services::ha_client::HomeAssistantService;
use crate::tools::registry;

pub struct McpServerState {
    pub ha_client: Arc<dyn HomeAssistantService>,
}

#[derive(Serialize)]
struct ToolListResponse {
    tools: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct ExecuteRequest {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Serialize)]
struct ExecuteResponse {
    success: bool,
    message: String,
}

async fn list_tools() -> Json<ToolListResponse> {
    let tools = registry::get_all_tools()
        .into_iter()
        .map(|tool| {
            serde_json::json!({
                "name": tool.name,
                "description": tool.description,
                "domain": tool.domain,
                "service": tool.service,
                "parameters": {
                    "type": tool.parameters.r#type,
                    "properties": tool.parameters.properties,
                    "required": tool.parameters.required,
                }
            })
        })
        .collect();

    Json(ToolListResponse { tools })
}

async fn execute_tool(
    AxumState(state): AxumState<Arc<McpServerState>>,
    Json(request): Json<ExecuteRequest>,
) -> (StatusCode, Json<ExecuteResponse>) {
    let tool = match registry::find_tool(&request.name) {
        Some(t) => t,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ExecuteResponse {
                    success: false,
                    message: format!("Unknown tool: {}", request.name),
                }),
            );
        }
    };

    let entity_id = match request.arguments.get("entity_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ExecuteResponse {
                    success: false,
                    message: "Missing required parameter: entity_id".to_string(),
                }),
            );
        }
    };

    // Build extra data by removing entity_id from arguments
    let extra_data = if let serde_json::Value::Object(mut map) = request.arguments.clone() {
        map.remove("entity_id");
        if map.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(map))
        }
    } else {
        None
    };

    match state
        .ha_client
        .call_service(&tool.domain, &tool.service, &entity_id, extra_data)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(ExecuteResponse {
                success: true,
                message: format!(
                    "{}.{} executed on {}",
                    tool.domain, tool.service, entity_id
                ),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ExecuteResponse {
                success: false,
                message: e,
            }),
        ),
    }
}

async fn health() -> &'static str {
    "ok"
}

pub fn create_router(state: Arc<McpServerState>) -> Router {
    Router::new()
        .route("/mcp/tools", get(list_tools))
        .route("/mcp/execute", post(execute_tool))
        .route("/mcp/health", get(health))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn start_mcp_server(
    state: Arc<McpServerState>,
    port: u16,
) -> Result<(), String> {
    let app = create_router(state);
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind MCP server to {}: {}", addr, e))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("MCP server error: {}", e))
}
