use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::services::ha_client::EntityState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub entity_id: String,
    pub domain: String,
    pub friendly_name: String,
    pub state: String,
    pub attributes: serde_json::Value,
    pub room: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub name: String,
    pub entity_ids: Vec<String>,
}

impl DeviceInfo {
    pub fn from_entity_state(entity: &EntityState, room: Option<String>) -> Self {
        let domain = entity
            .entity_id
            .split('.')
            .next()
            .unwrap_or("")
            .to_string();

        let friendly_name = entity
            .attributes
            .get("friendly_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&entity.entity_id)
            .to_string();

        Self {
            entity_id: entity.entity_id.clone(),
            domain,
            friendly_name,
            state: entity.state.clone(),
            attributes: entity.attributes.clone(),
            room,
        }
    }
}

/// Supported domains that we expose as controllable devices.
const SUPPORTED_DOMAINS: &[&str] = &["light", "switch", "climate", "media_player"];

pub struct DeviceStateCache {
    entities: RwLock<HashMap<String, EntityState>>,
    rooms: RwLock<HashMap<String, Vec<String>>>,
}

impl DeviceStateCache {
    pub fn new() -> Self {
        Self {
            entities: RwLock::new(HashMap::new()),
            rooms: RwLock::new(HashMap::new()),
        }
    }

    /// Populate from a full state dump from HA REST API.
    pub async fn populate(&self, states: Vec<EntityState>) {
        let mut entities = self.entities.write().await;
        entities.clear();
        for state in states {
            let domain = state.entity_id.split('.').next().unwrap_or("");
            if SUPPORTED_DOMAINS.contains(&domain) {
                entities.insert(state.entity_id.clone(), state);
            }
        }
    }

    /// Update a single entity (from WebSocket state_changed event).
    pub async fn update_entity(&self, entity_id: &str, state: EntityState) {
        let domain = entity_id.split('.').next().unwrap_or("");
        if SUPPORTED_DOMAINS.contains(&domain) {
            let mut entities = self.entities.write().await;
            entities.insert(entity_id.to_string(), state);
        }
    }

    /// Set room mappings (used when HA areas API is wired up).
    #[allow(dead_code)]
    pub async fn set_rooms(&self, rooms: HashMap<String, Vec<String>>) {
        let mut r = self.rooms.write().await;
        *r = rooms;
    }

    /// Get room for an entity.
    async fn get_room_for_entity(&self, entity_id: &str) -> Option<String> {
        let rooms = self.rooms.read().await;
        for (room_name, ids) in rooms.iter() {
            if ids.iter().any(|id| id == entity_id) {
                return Some(room_name.clone());
            }
        }
        None
    }

    /// Get all devices as DeviceInfo.
    pub async fn get_all_devices(&self) -> Vec<DeviceInfo> {
        let entities = self.entities.read().await;
        let mut devices = Vec::new();
        for entity in entities.values() {
            let room = self.get_room_for_entity(&entity.entity_id).await;
            devices.push(DeviceInfo::from_entity_state(entity, room));
        }
        devices.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
        devices
    }

    /// Get a single device state.
    pub async fn get_device(&self, entity_id: &str) -> Option<DeviceInfo> {
        let entities = self.entities.read().await;
        if let Some(entity) = entities.get(entity_id) {
            let room = self.get_room_for_entity(entity_id).await;
            Some(DeviceInfo::from_entity_state(entity, room))
        } else {
            None
        }
    }

    /// Get all rooms.
    pub async fn get_rooms(&self) -> Vec<RoomInfo> {
        let rooms = self.rooms.read().await;
        let mut result: Vec<RoomInfo> = rooms
            .iter()
            .map(|(name, ids)| RoomInfo {
                name: name.clone(),
                entity_ids: ids.clone(),
            })
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }

    pub async fn device_count(&self) -> usize {
        self.entities.read().await.len()
    }
}

pub fn new_shared_cache() -> Arc<DeviceStateCache> {
    Arc::new(DeviceStateCache::new())
}
