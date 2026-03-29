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
use tokio::sync::RwLock;

pub struct McpServerState {
    pub ha_client: Arc<RwLock<Arc<dyn HomeAssistantService>>>,
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
        .read()
        .await
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
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            println!("[mcp] Port {} already in use, skipping MCP server startup", port);
            return Ok(());
        }
        Err(e) => return Err(format!("Failed to bind MCP server to {}: {}", addr, e)),
    };

    println!("[mcp] MCP server listening on {}", addr);
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("MCP server error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ha_client::EntityState;
    use axum_test::TestServer;
    use serde_json::json;
    use std::sync::Mutex;

    struct MockHaService {
        calls: Mutex<Vec<(String, String, String, Option<serde_json::Value>)>>,
        should_fail: bool,
    }

    impl MockHaService {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                should_fail: true,
            }
        }
    }

    #[async_trait::async_trait]
    impl HomeAssistantService for MockHaService {
        async fn call_service(
            &self,
            domain: &str,
            service: &str,
            entity_id: &str,
            data: Option<serde_json::Value>,
        ) -> Result<(), String> {
            self.calls.lock().unwrap().push((
                domain.to_string(),
                service.to_string(),
                entity_id.to_string(),
                data,
            ));
            if self.should_fail {
                Err("HA unavailable".to_string())
            } else {
                Ok(())
            }
        }

        async fn get_state(&self, entity_id: &str) -> Result<EntityState, String> {
            Ok(EntityState {
                entity_id: entity_id.to_string(),
                state: "on".to_string(),
                attributes: json!({}),
                last_changed: String::new(),
                last_updated: String::new(),
            })
        }

        async fn get_all_states(&self) -> Result<Vec<EntityState>, String> {
            Ok(vec![])
        }

        async fn is_healthy(&self) -> bool {
            true
        }
    }

    fn test_server(mock: MockHaService) -> TestServer {
        let state = Arc::new(McpServerState {
            ha_client: Arc::new(RwLock::new(Arc::new(mock) as Arc<dyn HomeAssistantService>)),
        });
        let app = create_router(state);
        TestServer::new(app).unwrap()
    }

    // ──────────────────────────────────────────────
    // GET /mcp/health
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn health_returns_ok() {
        let server = test_server(MockHaService::new());
        let resp = server.get("/mcp/health").await;
        resp.assert_status_ok();
        resp.assert_text("ok");
    }

    // ──────────────────────────────────────────────
    // GET /mcp/tools
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn list_tools_returns_all_19() {
        let server = test_server(MockHaService::new());
        let resp = server.get("/mcp/tools").await;
        resp.assert_status_ok();
        let body: serde_json::Value = resp.json();
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 19);
    }

    #[tokio::test]
    async fn list_tools_format() {
        let server = test_server(MockHaService::new());
        let body: serde_json::Value = server.get("/mcp/tools").await.json();
        for tool in body["tools"].as_array().unwrap() {
            assert!(tool["name"].is_string());
            assert!(tool["description"].is_string());
            assert!(tool["domain"].is_string());
            assert!(tool["service"].is_string());
            assert!(tool["parameters"].is_object());
        }
    }

    // ──────────────────────────────────────────────
    // POST /mcp/execute — Light tools
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn execute_light_turn_on() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "light_turn_on",
                "arguments": {"entity_id": "light.living_room", "brightness": 200}
            }))
            .await;
        resp.assert_status_ok();
        let body: serde_json::Value = resp.json();
        assert_eq!(body["success"], true);
        assert!(body["message"]
            .as_str()
            .unwrap()
            .contains("light.turn_on"));
    }

    #[tokio::test]
    async fn execute_light_turn_off() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "light_turn_off",
                "arguments": {"entity_id": "light.bedroom"}
            }))
            .await;
        resp.assert_status_ok();
        let body: serde_json::Value = resp.json();
        assert_eq!(body["success"], true);
    }

    #[tokio::test]
    async fn execute_light_toggle() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "light_toggle",
                "arguments": {"entity_id": "light.porch"}
            }))
            .await;
        resp.assert_status_ok();
        let body: serde_json::Value = resp.json();
        assert_eq!(body["success"], true);
    }

    // ──────────────────────────────────────────────
    // POST /mcp/execute — Switch tools
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn execute_switch_turn_on() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "switch_turn_on",
                "arguments": {"entity_id": "switch.coffee_maker"}
            }))
            .await;
        resp.assert_status_ok();
        let body: serde_json::Value = resp.json();
        assert_eq!(body["success"], true);
        assert!(body["message"]
            .as_str()
            .unwrap()
            .contains("switch.turn_on"));
    }

    #[tokio::test]
    async fn execute_switch_turn_off() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "switch_turn_off",
                "arguments": {"entity_id": "switch.fan"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    #[tokio::test]
    async fn execute_switch_toggle() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "switch_toggle",
                "arguments": {"entity_id": "switch.outlet"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    // ──────────────────────────────────────────────
    // POST /mcp/execute — Climate tools
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn execute_climate_set_temperature() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "climate_set_temperature",
                "arguments": {"entity_id": "climate.thermostat", "temperature": 72}
            }))
            .await;
        resp.assert_status_ok();
        let body: serde_json::Value = resp.json();
        assert_eq!(body["success"], true);
        assert!(body["message"]
            .as_str()
            .unwrap()
            .contains("climate.set_temperature"));
    }

    #[tokio::test]
    async fn execute_climate_set_hvac_mode() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "climate_set_hvac_mode",
                "arguments": {"entity_id": "climate.thermostat", "hvac_mode": "cool"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    #[tokio::test]
    async fn execute_climate_set_fan_mode() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "climate_set_fan_mode",
                "arguments": {"entity_id": "climate.thermostat", "fan_mode": "high"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    #[tokio::test]
    async fn execute_climate_turn_on() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "climate_turn_on",
                "arguments": {"entity_id": "climate.bedroom_ac"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    #[tokio::test]
    async fn execute_climate_turn_off() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "climate_turn_off",
                "arguments": {"entity_id": "climate.bedroom_ac"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    // ──────────────────────────────────────────────
    // POST /mcp/execute — Media player tools
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn execute_media_player_play_media() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "media_player_play_media",
                "arguments": {
                    "entity_id": "media_player.speaker",
                    "media_content_id": "spotify:track:abc",
                    "media_content_type": "music"
                }
            }))
            .await;
        resp.assert_status_ok();
        let body: serde_json::Value = resp.json();
        assert_eq!(body["success"], true);
        assert!(body["message"]
            .as_str()
            .unwrap()
            .contains("media_player.play_media"));
    }

    #[tokio::test]
    async fn execute_media_player_media_pause() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "media_player_media_pause",
                "arguments": {"entity_id": "media_player.speaker"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    #[tokio::test]
    async fn execute_media_player_media_play() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "media_player_media_play",
                "arguments": {"entity_id": "media_player.speaker"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    #[tokio::test]
    async fn execute_media_player_media_stop() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "media_player_media_stop",
                "arguments": {"entity_id": "media_player.speaker"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    #[tokio::test]
    async fn execute_media_player_volume_set() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "media_player_volume_set",
                "arguments": {"entity_id": "media_player.speaker", "volume_level": 0.75}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    #[tokio::test]
    async fn execute_media_player_volume_up() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "media_player_volume_up",
                "arguments": {"entity_id": "media_player.speaker"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    #[tokio::test]
    async fn execute_media_player_volume_down() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "media_player_volume_down",
                "arguments": {"entity_id": "media_player.speaker"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    #[tokio::test]
    async fn execute_media_player_select_source() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "media_player_select_source",
                "arguments": {"entity_id": "media_player.tv", "source": "HDMI 2"}
            }))
            .await;
        resp.assert_status_ok();
        assert_eq!(resp.json::<serde_json::Value>()["success"], true);
    }

    // ──────────────────────────────────────────────
    // POST /mcp/execute — Error cases
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn execute_unknown_tool_returns_404() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "fake_tool",
                "arguments": {"entity_id": "light.test"}
            }))
            .await;
        resp.assert_status(StatusCode::NOT_FOUND);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["success"], false);
        assert!(body["message"].as_str().unwrap().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn execute_missing_entity_id_returns_400() {
        let server = test_server(MockHaService::new());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "light_turn_on",
                "arguments": {"brightness": 128}
            }))
            .await;
        resp.assert_status(StatusCode::BAD_REQUEST);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["success"], false);
        assert!(body["message"].as_str().unwrap().contains("entity_id"));
    }

    #[tokio::test]
    async fn execute_ha_failure_returns_500() {
        let server = test_server(MockHaService::failing());
        let resp = server
            .post("/mcp/execute")
            .json(&json!({
                "name": "light_turn_on",
                "arguments": {"entity_id": "light.test"}
            }))
            .await;
        resp.assert_status(StatusCode::INTERNAL_SERVER_ERROR);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["success"], false);
    }
}
