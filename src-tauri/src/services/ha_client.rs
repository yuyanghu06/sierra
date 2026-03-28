use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityState {
    pub entity_id: String,
    pub state: String,
    pub attributes: serde_json::Value,
    pub last_changed: String,
    pub last_updated: String,
}

#[async_trait::async_trait]
pub trait HomeAssistantService: Send + Sync {
    async fn call_service(
        &self,
        domain: &str,
        service: &str,
        entity_id: &str,
        data: Option<serde_json::Value>,
    ) -> Result<(), String>;

    async fn get_state(&self, entity_id: &str) -> Result<EntityState, String>;

    async fn get_all_states(&self) -> Result<Vec<EntityState>, String>;

    async fn is_healthy(&self) -> bool;
}

pub struct HaRestClient {
    client: Client,
    base_url: String,
    token: String,
}

impl HaRestClient {
    pub fn new(base_url: String, token: String) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }
}

#[async_trait::async_trait]
impl HomeAssistantService for HaRestClient {
    async fn call_service(
        &self,
        domain: &str,
        service: &str,
        entity_id: &str,
        data: Option<serde_json::Value>,
    ) -> Result<(), String> {
        let url = format!("{}/api/services/{}/{}", self.base_url, domain, service);

        let mut body = match data {
            Some(serde_json::Value::Object(map)) => serde_json::Value::Object(map),
            Some(_) => serde_json::json!({}),
            None => serde_json::json!({}),
        };

        if let serde_json::Value::Object(ref mut map) = body {
            map.insert(
                "entity_id".to_string(),
                serde_json::Value::String(entity_id.to_string()),
            );
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HA service call failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("HA returned {} for {}.{}: {}", status, domain, service, text));
        }

        Ok(())
    }

    async fn get_state(&self, entity_id: &str) -> Result<EntityState, String> {
        let url = format!("{}/api/states/{}", self.base_url, entity_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| format!("HA get_state failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("HA returned {} for state {}: {}", status, entity_id, text));
        }

        response
            .json::<EntityState>()
            .await
            .map_err(|e| format!("Failed to parse entity state: {}", e))
    }

    async fn get_all_states(&self) -> Result<Vec<EntityState>, String> {
        let url = format!("{}/api/states", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| format!("HA get_all_states failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("HA returned {} for all states: {}", status, text));
        }

        response
            .json::<Vec<EntityState>>()
            .await
            .map_err(|e| format!("Failed to parse states: {}", e))
    }

    async fn is_healthy(&self) -> bool {
        let url = format!("{}/api/", self.base_url);
        self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}
