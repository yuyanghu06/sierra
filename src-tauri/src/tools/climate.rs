use serde_json::json;

use super::{ToolDefinition, ToolParameters};

pub fn tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "climate_set_temperature".to_string(),
            description: "Set the target temperature on a thermostat or climate device. Use this \
                when the user specifies a desired temperature. You can set a single target \
                temperature, or a high/low range for devices that support it. Temperature units \
                depend on the Home Assistant configuration (typically Fahrenheit in the US, \
                Celsius elsewhere)."
                .to_string(),
            domain: "climate".to_string(),
            service: "set_temperature".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the climate device (e.g. climate.thermostat)"
                    },
                    "temperature": {
                        "type": "number",
                        "description": "Target temperature. Used when the device has a single setpoint"
                    },
                    "target_temp_high": {
                        "type": "number",
                        "description": "Upper bound of the target temperature range. Used with target_temp_low for devices in auto/heat_cool mode"
                    },
                    "target_temp_low": {
                        "type": "number",
                        "description": "Lower bound of the target temperature range. Used with target_temp_high for devices in auto/heat_cool mode"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "climate_set_hvac_mode".to_string(),
            description: "Set the HVAC mode on a thermostat. Use this when the user wants to \
                change between heating, cooling, auto, or off modes."
                .to_string(),
            domain: "climate".to_string(),
            service: "set_hvac_mode".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the climate device (e.g. climate.thermostat)"
                    },
                    "hvac_mode": {
                        "type": "string",
                        "description": "The HVAC mode to set",
                        "enum": ["heat", "cool", "auto", "off", "heat_cool", "fan_only", "dry"]
                    }
                }),
                required: vec!["entity_id".to_string(), "hvac_mode".to_string()],
            },
        },
        ToolDefinition {
            name: "climate_set_fan_mode".to_string(),
            description: "Set the fan mode on a climate device. Use this when the user wants to \
                adjust the fan speed or behavior."
                .to_string(),
            domain: "climate".to_string(),
            service: "set_fan_mode".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the climate device (e.g. climate.thermostat)"
                    },
                    "fan_mode": {
                        "type": "string",
                        "description": "The fan mode to set",
                        "enum": ["auto", "low", "medium", "high", "off"]
                    }
                }),
                required: vec!["entity_id".to_string(), "fan_mode".to_string()],
            },
        },
        ToolDefinition {
            name: "climate_turn_on".to_string(),
            description: "Turn on a climate device. Use this to start a thermostat or HVAC system \
                that is currently off."
                .to_string(),
            domain: "climate".to_string(),
            service: "turn_on".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the climate device (e.g. climate.thermostat)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "climate_turn_off".to_string(),
            description: "Turn off a climate device. Use this to stop a thermostat or HVAC system."
                .to_string(),
            domain: "climate".to_string(),
            service: "turn_off".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the climate device (e.g. climate.thermostat)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
    ]
}
