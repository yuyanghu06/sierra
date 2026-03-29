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
        // Accepts 2xx, 401, and 403 as "HA is alive" — matching process_manager health check.
        // A 401 means HA is running but the token is missing/invalid; the service is still up.
        // Use check_connection() when you need to verify auth validity.
        let url = format!("{}/api/", self.base_url);
        self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map(|r| {
                let s = r.status().as_u16();
                s < 500 && s != 404
            })
            .unwrap_or(false)
    }
}

/// Detailed connection status returned by `test_ha_connection`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum HaConnectionStatus {
    /// Token valid, API responding normally.
    Connected,
    /// HA is running but onboarding hasn't been completed yet.
    /// User must visit the HA URL in a browser to create an account.
    NeedsOnboarding,
    /// HA is running but the token is missing or invalid (HTTP 401/403).
    InvalidToken,
    /// HA is not reachable at the given URL.
    Unreachable,
}

impl HaRestClient {
    /// Detailed connection check — distinguishes between not running, bad token, and onboarding.
    pub async fn check_connection(&self) -> HaConnectionStatus {
        let api_url = format!("{}/api/", self.base_url);

        let response = match self
            .client
            .get(&api_url)
            .header("Authorization", self.auth_header())
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(r) => r,
            Err(_) => return HaConnectionStatus::Unreachable,
        };

        match response.status().as_u16() {
            200 => HaConnectionStatus::Connected,
            401 | 403 => {
                // HA is up. Check if onboarding is still required.
                // GET /api/onboarding returns {"done": [...]} without auth.
                // If "user" is absent from the done list, no real user account exists.
                let onboarding_url = format!("{}/api/onboarding", self.base_url);
                let user_step_done = async {
                    let resp = self
                        .client
                        .get(&onboarding_url)
                        .timeout(std::time::Duration::from_secs(3))
                        .send()
                        .await
                        .ok()?;

                    if !resp.status().is_success() {
                        return Some(true); // can't read onboarding, assume done
                    }

                    let json: serde_json::Value = resp.json().await.ok()?;
                    let done_arr = json.get("done")?.as_array()?;
                    Some(done_arr.iter().any(|s| s.as_str() == Some("user")))
                }
                .await
                .unwrap_or(true); // if we can't tell, assume onboarding is done

                if user_step_done {
                    HaConnectionStatus::InvalidToken
                } else {
                    HaConnectionStatus::NeedsOnboarding
                }
            }
            _ => HaConnectionStatus::InvalidToken,
        }
    }
}
