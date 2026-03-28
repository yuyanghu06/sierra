use crate::devices::DeviceStateCache;
use std::sync::Arc;

/// The system prompt template, embedded at compile time from prompts/system-prompt.md.
const SYSTEM_PROMPT_TEMPLATE: &str = include_str!("../../prompts/system-prompt.md");

/// Build the system prompt with the current device list injected.
pub async fn build_system_prompt(device_cache: &Arc<DeviceStateCache>) -> String {
    let devices = device_cache.get_all_devices().await;

    let device_list = if devices.is_empty() {
        "No devices are currently available. Home Assistant may not be connected.".to_string()
    } else {
        devices
            .iter()
            .map(|d| {
                let room = d
                    .room
                    .as_deref()
                    .unwrap_or("Unassigned");
                format!("- {} ({}) [{}] — state: {}", d.entity_id, d.friendly_name, room, d.state)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    SYSTEM_PROMPT_TEMPLATE.replace("{{DEVICE_LIST}}", &device_list)
}
