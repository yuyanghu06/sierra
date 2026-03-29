use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::ha_client::EntityState;

#[derive(Debug, Deserialize)]
struct WsMessage {
    r#type: String,
    #[serde(default)]
    event: Option<WsEvent>,
}

#[derive(Debug, Deserialize)]
struct WsEvent {
    #[serde(default)]
    data: Option<WsEventData>,
}

#[derive(Debug, Deserialize)]
struct WsEventData {
    entity_id: Option<String>,
    new_state: Option<EntityState>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceStateChanged {
    pub entity_id: String,
    pub new_state: EntityState,
}

type StateCallback = Arc<dyn Fn(DeviceStateChanged) + Send + Sync>;

pub struct HaWebSocketClient {
    ws_url: String,
    token: String,
    connected: Arc<RwLock<bool>>,
}

impl HaWebSocketClient {
    pub fn new(base_url: &str, token: &str) -> Self {
        let ws_url = base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://")
            .trim_end_matches('/')
            .to_string()
            + "/api/websocket";

        Self {
            ws_url,
            token: token.to_string(),
            connected: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn subscribe(
        &self,
        on_state_change: StateCallback,
    ) -> Result<tokio::task::JoinHandle<()>, String> {
        let (ws_stream, _) = connect_async(&self.ws_url)
            .await
            .map_err(|e| format!("WebSocket connection failed: {}", e))?;

        let (mut write, mut read) = ws_stream.split();

        // Wait for auth_required
        if let Some(Ok(msg)) = read.next().await {
            let text = msg.to_text().unwrap_or("");
            let parsed: serde_json::Value =
                serde_json::from_str(text).unwrap_or(json!({}));
            if parsed.get("type").and_then(|t| t.as_str()) != Some("auth_required") {
                return Err("Expected auth_required from HA WebSocket".to_string());
            }
        }

        // Send auth
        let auth_msg = json!({
            "type": "auth",
            "access_token": self.token
        });
        write
            .send(Message::Text(auth_msg.to_string().into()))
            .await
            .map_err(|e| format!("Failed to send auth: {}", e))?;

        // Wait for auth_ok
        if let Some(Ok(msg)) = read.next().await {
            let text = msg.to_text().unwrap_or("");
            let parsed: serde_json::Value =
                serde_json::from_str(text).unwrap_or(json!({}));
            match parsed.get("type").and_then(|t| t.as_str()) {
                Some("auth_ok") => {}
                Some("auth_invalid") => {
                    return Err("HA WebSocket authentication failed: invalid token".to_string());
                }
                _ => {
                    return Err("Unexpected response during HA WebSocket auth".to_string());
                }
            }
        }

        // Subscribe to state_changed events
        let subscribe_msg = json!({
            "id": 1,
            "type": "subscribe_events",
            "event_type": "state_changed"
        });
        write
            .send(Message::Text(subscribe_msg.to_string().into()))
            .await
            .map_err(|e| format!("Failed to subscribe to events: {}", e))?;

        let connected = self.connected.clone();
        *connected.write().await = true;

        let handle = tokio::spawn(async move {
            while let Some(Ok(msg)) = read.next().await {
                let text = match msg.to_text() {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                let parsed: WsMessage = match serde_json::from_str(text) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                if parsed.r#type != "event" {
                    continue;
                }

                if let Some(event) = parsed.event {
                    if let Some(data) = event.data {
                        if let (Some(entity_id), Some(new_state)) =
                            (data.entity_id, data.new_state)
                        {
                            on_state_change(DeviceStateChanged {
                                entity_id,
                                new_state,
                            });
                        }
                    }
                }
            }

            *connected.write().await = false;
        });

        Ok(handle)
    }
}
