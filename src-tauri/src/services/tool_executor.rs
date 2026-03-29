use std::sync::Arc;

use crate::services::ha_client::HomeAssistantService;
use crate::services::llm::ToolExecutor;
use crate::tools::registry;

pub struct HaToolExecutor {
    ha_client: Arc<dyn HomeAssistantService>,
}

impl HaToolExecutor {
    pub fn new(ha_client: Arc<dyn HomeAssistantService>) -> Self {
        Self { ha_client }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for HaToolExecutor {
    async fn execute(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, String> {
        let tool = registry::find_tool(tool_name)
            .ok_or_else(|| format!("Unknown tool: {}", tool_name))?;

        let entity_id = arguments
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: entity_id".to_string())?;

        // Build extra data by removing entity_id from arguments
        let extra_data = if let serde_json::Value::Object(mut map) = arguments.clone() {
            map.remove("entity_id");
            if map.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(map))
            }
        } else {
            None
        };

        self.ha_client
            .call_service(&tool.domain, &tool.service, entity_id, extra_data)
            .await?;

        Ok(format!(
            "Successfully executed {}.{} on {}",
            tool.domain, tool.service, entity_id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ha_client::EntityState;
    use serde_json::json;
    use std::sync::Mutex;

    /// Records every call_service invocation for assertion.
    struct MockHaService {
        calls: Mutex<Vec<ServiceCall>>,
        should_fail: bool,
    }

    #[derive(Debug, Clone)]
    struct ServiceCall {
        domain: String,
        service: String,
        entity_id: String,
        data: Option<serde_json::Value>,
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

        fn last_call(&self) -> ServiceCall {
            self.calls.lock().unwrap().last().unwrap().clone()
        }

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
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
            self.calls.lock().unwrap().push(ServiceCall {
                domain: domain.to_string(),
                service: service.to_string(),
                entity_id: entity_id.to_string(),
                data,
            });
            if self.should_fail {
                Err("HA returned 503 for service call".to_string())
            } else {
                Ok(())
            }
        }

        async fn get_state(&self, entity_id: &str) -> Result<EntityState, String> {
            Ok(EntityState {
                entity_id: entity_id.to_string(),
                state: "on".to_string(),
                attributes: json!({}),
                last_changed: "2026-01-01T00:00:00Z".to_string(),
                last_updated: "2026-01-01T00:00:00Z".to_string(),
            })
        }

        async fn get_all_states(&self) -> Result<Vec<EntityState>, String> {
            Ok(vec![])
        }

        async fn is_healthy(&self) -> bool {
            !self.should_fail
        }
    }

    fn make_executor(mock: MockHaService) -> (HaToolExecutor, Arc<MockHaService>) {
        let mock = Arc::new(mock);
        let executor = HaToolExecutor::new(mock.clone());
        (executor, mock)
    }

    // ──────────────────────────────────────────────
    // Error handling
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn unknown_tool_returns_error() {
        let (executor, _mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.test"});
        let result = executor.execute("nonexistent_tool", &args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn missing_entity_id_returns_error() {
        let (executor, _mock) = make_executor(MockHaService::new());
        let args = json!({"brightness": 128});
        let result = executor.execute("light_turn_on", &args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("entity_id"));
    }

    #[tokio::test]
    async fn null_entity_id_returns_error() {
        let (executor, _mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": null});
        let result = executor.execute("light_turn_on", &args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("entity_id"));
    }

    #[tokio::test]
    async fn numeric_entity_id_returns_error() {
        let (executor, _mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": 42});
        let result = executor.execute("light_turn_on", &args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn ha_failure_propagates_error() {
        let (executor, _mock) = make_executor(MockHaService::failing());
        let args = json!({"entity_id": "light.living_room"});
        let result = executor.execute("light_turn_on", &args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("503"));
    }

    #[tokio::test]
    async fn empty_arguments_object_returns_error() {
        let (executor, _mock) = make_executor(MockHaService::new());
        let args = json!({});
        let result = executor.execute("light_turn_on", &args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("entity_id"));
    }

    // ──────────────────────────────────────────────
    // Light domain (3 tools)
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn light_turn_on_minimal() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.living_room"});
        let result = executor.execute("light_turn_on", &args).await.unwrap();
        assert!(result.contains("light.turn_on"));
        assert!(result.contains("light.living_room"));
        let call = mock.last_call();
        assert_eq!(call.domain, "light");
        assert_eq!(call.service, "turn_on");
        assert_eq!(call.entity_id, "light.living_room");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn light_turn_on_with_brightness() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.bedroom", "brightness": 128});
        let result = executor.execute("light_turn_on", &args).await.unwrap();
        assert!(result.contains("light.turn_on"));
        let call = mock.last_call();
        assert_eq!(call.domain, "light");
        assert_eq!(call.service, "turn_on");
        assert_eq!(call.entity_id, "light.bedroom");
        let data = call.data.unwrap();
        assert_eq!(data["brightness"], 128);
    }

    #[tokio::test]
    async fn light_turn_on_with_color_temp() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.kitchen", "color_temp": 350});
        executor.execute("light_turn_on", &args).await.unwrap();
        let call = mock.last_call();
        let data = call.data.unwrap();
        assert_eq!(data["color_temp"], 350);
    }

    #[tokio::test]
    async fn light_turn_on_with_rgb_color() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.desk", "rgb_color": [255, 0, 128]});
        executor.execute("light_turn_on", &args).await.unwrap();
        let call = mock.last_call();
        let data = call.data.unwrap();
        assert_eq!(data["rgb_color"], json!([255, 0, 128]));
    }

    #[tokio::test]
    async fn light_turn_on_with_transition() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.hallway", "transition": 2.5});
        executor.execute("light_turn_on", &args).await.unwrap();
        let call = mock.last_call();
        let data = call.data.unwrap();
        assert_eq!(data["transition"], 2.5);
    }

    #[tokio::test]
    async fn light_turn_on_with_effect() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.strip", "effect": "colorloop"});
        executor.execute("light_turn_on", &args).await.unwrap();
        let call = mock.last_call();
        let data = call.data.unwrap();
        assert_eq!(data["effect"], "colorloop");
    }

    #[tokio::test]
    async fn light_turn_on_with_all_params() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({
            "entity_id": "light.living_room",
            "brightness": 200,
            "color_temp": 300,
            "rgb_color": [255, 200, 100],
            "transition": 1.0,
            "effect": "random"
        });
        executor.execute("light_turn_on", &args).await.unwrap();
        let call = mock.last_call();
        assert_eq!(call.entity_id, "light.living_room");
        let data = call.data.unwrap();
        assert_eq!(data["brightness"], 200);
        assert_eq!(data["color_temp"], 300);
        assert_eq!(data["rgb_color"], json!([255, 200, 100]));
        assert_eq!(data["transition"], 1.0);
        assert_eq!(data["effect"], "random");
    }

    #[tokio::test]
    async fn light_turn_off_minimal() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.bedroom"});
        let result = executor.execute("light_turn_off", &args).await.unwrap();
        assert!(result.contains("light.turn_off"));
        let call = mock.last_call();
        assert_eq!(call.domain, "light");
        assert_eq!(call.service, "turn_off");
        assert_eq!(call.entity_id, "light.bedroom");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn light_turn_off_with_transition() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.bedroom", "transition": 5.0});
        executor.execute("light_turn_off", &args).await.unwrap();
        let call = mock.last_call();
        assert_eq!(call.service, "turn_off");
        let data = call.data.unwrap();
        assert_eq!(data["transition"], 5.0);
    }

    #[tokio::test]
    async fn light_toggle() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.porch"});
        let result = executor.execute("light_toggle", &args).await.unwrap();
        assert!(result.contains("light.toggle"));
        let call = mock.last_call();
        assert_eq!(call.domain, "light");
        assert_eq!(call.service, "toggle");
        assert_eq!(call.entity_id, "light.porch");
        assert!(call.data.is_none());
    }

    // ──────────────────────────────────────────────
    // Switch domain (3 tools)
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn switch_turn_on() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "switch.coffee_maker"});
        let result = executor.execute("switch_turn_on", &args).await.unwrap();
        assert!(result.contains("switch.turn_on"));
        let call = mock.last_call();
        assert_eq!(call.domain, "switch");
        assert_eq!(call.service, "turn_on");
        assert_eq!(call.entity_id, "switch.coffee_maker");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn switch_turn_off() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "switch.fan"});
        let result = executor.execute("switch_turn_off", &args).await.unwrap();
        assert!(result.contains("switch.turn_off"));
        let call = mock.last_call();
        assert_eq!(call.domain, "switch");
        assert_eq!(call.service, "turn_off");
        assert_eq!(call.entity_id, "switch.fan");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn switch_toggle() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "switch.outlet"});
        let result = executor.execute("switch_toggle", &args).await.unwrap();
        assert!(result.contains("switch.toggle"));
        let call = mock.last_call();
        assert_eq!(call.domain, "switch");
        assert_eq!(call.service, "toggle");
        assert_eq!(call.entity_id, "switch.outlet");
        assert!(call.data.is_none());
    }

    // ──────────────────────────────────────────────
    // Climate domain (5 tools)
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn climate_set_temperature_single() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "temperature": 72.0});
        let result = executor
            .execute("climate_set_temperature", &args)
            .await
            .unwrap();
        assert!(result.contains("climate.set_temperature"));
        let call = mock.last_call();
        assert_eq!(call.domain, "climate");
        assert_eq!(call.service, "set_temperature");
        assert_eq!(call.entity_id, "climate.thermostat");
        let data = call.data.unwrap();
        assert_eq!(data["temperature"], 72.0);
    }

    #[tokio::test]
    async fn climate_set_temperature_range() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({
            "entity_id": "climate.thermostat",
            "target_temp_high": 76.0,
            "target_temp_low": 68.0
        });
        executor
            .execute("climate_set_temperature", &args)
            .await
            .unwrap();
        let call = mock.last_call();
        let data = call.data.unwrap();
        assert_eq!(data["target_temp_high"], 76.0);
        assert_eq!(data["target_temp_low"], 68.0);
    }

    #[tokio::test]
    async fn climate_set_temperature_entity_only() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat"});
        executor
            .execute("climate_set_temperature", &args)
            .await
            .unwrap();
        let call = mock.last_call();
        assert_eq!(call.service, "set_temperature");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn climate_set_hvac_mode_heat() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "hvac_mode": "heat"});
        let result = executor
            .execute("climate_set_hvac_mode", &args)
            .await
            .unwrap();
        assert!(result.contains("climate.set_hvac_mode"));
        let call = mock.last_call();
        assert_eq!(call.domain, "climate");
        assert_eq!(call.service, "set_hvac_mode");
        let data = call.data.unwrap();
        assert_eq!(data["hvac_mode"], "heat");
    }

    #[tokio::test]
    async fn climate_set_hvac_mode_cool() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "hvac_mode": "cool"});
        executor
            .execute("climate_set_hvac_mode", &args)
            .await
            .unwrap();
        let call = mock.last_call();
        let data = call.data.unwrap();
        assert_eq!(data["hvac_mode"], "cool");
    }

    #[tokio::test]
    async fn climate_set_hvac_mode_auto() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "hvac_mode": "auto"});
        executor
            .execute("climate_set_hvac_mode", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["hvac_mode"], "auto");
    }

    #[tokio::test]
    async fn climate_set_hvac_mode_off() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "hvac_mode": "off"});
        executor
            .execute("climate_set_hvac_mode", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["hvac_mode"], "off");
    }

    #[tokio::test]
    async fn climate_set_hvac_mode_heat_cool() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "hvac_mode": "heat_cool"});
        executor
            .execute("climate_set_hvac_mode", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["hvac_mode"], "heat_cool");
    }

    #[tokio::test]
    async fn climate_set_hvac_mode_fan_only() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "hvac_mode": "fan_only"});
        executor
            .execute("climate_set_hvac_mode", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["hvac_mode"], "fan_only");
    }

    #[tokio::test]
    async fn climate_set_hvac_mode_dry() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "hvac_mode": "dry"});
        executor
            .execute("climate_set_hvac_mode", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["hvac_mode"], "dry");
    }

    #[tokio::test]
    async fn climate_set_fan_mode_auto() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "fan_mode": "auto"});
        let result = executor
            .execute("climate_set_fan_mode", &args)
            .await
            .unwrap();
        assert!(result.contains("climate.set_fan_mode"));
        let call = mock.last_call();
        assert_eq!(call.service, "set_fan_mode");
        let data = call.data.unwrap();
        assert_eq!(data["fan_mode"], "auto");
    }

    #[tokio::test]
    async fn climate_set_fan_mode_low() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "fan_mode": "low"});
        executor
            .execute("climate_set_fan_mode", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["fan_mode"], "low");
    }

    #[tokio::test]
    async fn climate_set_fan_mode_medium() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "fan_mode": "medium"});
        executor
            .execute("climate_set_fan_mode", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["fan_mode"], "medium");
    }

    #[tokio::test]
    async fn climate_set_fan_mode_high() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "fan_mode": "high"});
        executor
            .execute("climate_set_fan_mode", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["fan_mode"], "high");
    }

    #[tokio::test]
    async fn climate_set_fan_mode_off() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.thermostat", "fan_mode": "off"});
        executor
            .execute("climate_set_fan_mode", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["fan_mode"], "off");
    }

    #[tokio::test]
    async fn climate_turn_on() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.bedroom_ac"});
        let result = executor.execute("climate_turn_on", &args).await.unwrap();
        assert!(result.contains("climate.turn_on"));
        let call = mock.last_call();
        assert_eq!(call.domain, "climate");
        assert_eq!(call.service, "turn_on");
        assert_eq!(call.entity_id, "climate.bedroom_ac");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn climate_turn_off() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "climate.bedroom_ac"});
        let result = executor.execute("climate_turn_off", &args).await.unwrap();
        assert!(result.contains("climate.turn_off"));
        let call = mock.last_call();
        assert_eq!(call.domain, "climate");
        assert_eq!(call.service, "turn_off");
        assert_eq!(call.entity_id, "climate.bedroom_ac");
        assert!(call.data.is_none());
    }

    // ──────────────────────────────────────────────
    // Media player domain (8 tools)
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn media_player_play_media() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({
            "entity_id": "media_player.living_room_speaker",
            "media_content_id": "spotify:playlist:abc123",
            "media_content_type": "playlist"
        });
        let result = executor
            .execute("media_player_play_media", &args)
            .await
            .unwrap();
        assert!(result.contains("media_player.play_media"));
        let call = mock.last_call();
        assert_eq!(call.domain, "media_player");
        assert_eq!(call.service, "play_media");
        assert_eq!(call.entity_id, "media_player.living_room_speaker");
        let data = call.data.unwrap();
        assert_eq!(data["media_content_id"], "spotify:playlist:abc123");
        assert_eq!(data["media_content_type"], "playlist");
    }

    #[tokio::test]
    async fn media_player_play_media_music() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({
            "entity_id": "media_player.bedroom",
            "media_content_id": "http://example.com/song.mp3",
            "media_content_type": "music"
        });
        executor
            .execute("media_player_play_media", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["media_content_type"], "music");
    }

    #[tokio::test]
    async fn media_player_media_pause() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "media_player.living_room_speaker"});
        let result = executor
            .execute("media_player_media_pause", &args)
            .await
            .unwrap();
        assert!(result.contains("media_player.media_pause"));
        let call = mock.last_call();
        assert_eq!(call.domain, "media_player");
        assert_eq!(call.service, "media_pause");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn media_player_media_play() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "media_player.living_room_speaker"});
        let result = executor
            .execute("media_player_media_play", &args)
            .await
            .unwrap();
        assert!(result.contains("media_player.media_play"));
        let call = mock.last_call();
        assert_eq!(call.domain, "media_player");
        assert_eq!(call.service, "media_play");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn media_player_media_stop() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "media_player.living_room_speaker"});
        let result = executor
            .execute("media_player_media_stop", &args)
            .await
            .unwrap();
        assert!(result.contains("media_player.media_stop"));
        let call = mock.last_call();
        assert_eq!(call.domain, "media_player");
        assert_eq!(call.service, "media_stop");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn media_player_volume_set() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({
            "entity_id": "media_player.living_room_speaker",
            "volume_level": 0.5
        });
        let result = executor
            .execute("media_player_volume_set", &args)
            .await
            .unwrap();
        assert!(result.contains("media_player.volume_set"));
        let call = mock.last_call();
        assert_eq!(call.domain, "media_player");
        assert_eq!(call.service, "volume_set");
        let data = call.data.unwrap();
        assert_eq!(data["volume_level"], 0.5);
    }

    #[tokio::test]
    async fn media_player_volume_set_min() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({
            "entity_id": "media_player.speaker",
            "volume_level": 0.0
        });
        executor
            .execute("media_player_volume_set", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["volume_level"], 0.0);
    }

    #[tokio::test]
    async fn media_player_volume_set_max() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({
            "entity_id": "media_player.speaker",
            "volume_level": 1.0
        });
        executor
            .execute("media_player_volume_set", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["volume_level"], 1.0);
    }

    #[tokio::test]
    async fn media_player_volume_up() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "media_player.living_room_speaker"});
        let result = executor
            .execute("media_player_volume_up", &args)
            .await
            .unwrap();
        assert!(result.contains("media_player.volume_up"));
        let call = mock.last_call();
        assert_eq!(call.domain, "media_player");
        assert_eq!(call.service, "volume_up");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn media_player_volume_down() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "media_player.living_room_speaker"});
        let result = executor
            .execute("media_player_volume_down", &args)
            .await
            .unwrap();
        assert!(result.contains("media_player.volume_down"));
        let call = mock.last_call();
        assert_eq!(call.domain, "media_player");
        assert_eq!(call.service, "volume_down");
        assert!(call.data.is_none());
    }

    #[tokio::test]
    async fn media_player_select_source() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({
            "entity_id": "media_player.living_room_speaker",
            "source": "Bluetooth"
        });
        let result = executor
            .execute("media_player_select_source", &args)
            .await
            .unwrap();
        assert!(result.contains("media_player.select_source"));
        let call = mock.last_call();
        assert_eq!(call.domain, "media_player");
        assert_eq!(call.service, "select_source");
        let data = call.data.unwrap();
        assert_eq!(data["source"], "Bluetooth");
    }

    #[tokio::test]
    async fn media_player_select_source_hdmi() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({
            "entity_id": "media_player.tv",
            "source": "HDMI 1"
        });
        executor
            .execute("media_player_select_source", &args)
            .await
            .unwrap();
        let data = mock.last_call().data.unwrap();
        assert_eq!(data["source"], "HDMI 1");
    }

    // ──────────────────────────────────────────────
    // Cross-domain: extra params are stripped correctly
    // ──────────────────────────────────────────────

    #[tokio::test]
    async fn entity_id_stripped_from_extra_data() {
        let (executor, mock) = make_executor(MockHaService::new());
        let args = json!({"entity_id": "light.test", "brightness": 100});
        executor.execute("light_turn_on", &args).await.unwrap();
        let call = mock.last_call();
        let data = call.data.unwrap();
        assert!(data.get("entity_id").is_none());
        assert_eq!(data["brightness"], 100);
    }

    #[tokio::test]
    async fn multiple_tools_sequential() {
        let (executor, mock) = make_executor(MockHaService::new());

        executor
            .execute("light_turn_on", &json!({"entity_id": "light.a"}))
            .await
            .unwrap();
        executor
            .execute("switch_turn_off", &json!({"entity_id": "switch.b"}))
            .await
            .unwrap();
        executor
            .execute(
                "climate_set_temperature",
                &json!({"entity_id": "climate.c", "temperature": 68}),
            )
            .await
            .unwrap();

        assert_eq!(mock.call_count(), 3);
        let calls = mock.calls.lock().unwrap();
        assert_eq!(calls[0].domain, "light");
        assert_eq!(calls[1].domain, "switch");
        assert_eq!(calls[2].domain, "climate");
    }
}
