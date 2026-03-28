use serde_json::json;

use super::{ToolDefinition, ToolParameters};

pub fn tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "switch_turn_on".to_string(),
            description: "Turn on a switch. Use this for binary on/off devices like smart plugs, \
                outlets, or simple switches that don't have brightness or color controls."
                .to_string(),
            domain: "switch".to_string(),
            service: "turn_on".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the switch (e.g. switch.coffee_maker)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "switch_turn_off".to_string(),
            description: "Turn off a switch. Use this to power off a smart plug, outlet, or \
                simple switch."
                .to_string(),
            domain: "switch".to_string(),
            service: "turn_off".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the switch (e.g. switch.coffee_maker)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "switch_toggle".to_string(),
            description: "Toggle a switch between on and off. Use this when the user wants to \
                flip a switch to the opposite state."
                .to_string(),
            domain: "switch".to_string(),
            service: "toggle".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the switch (e.g. switch.coffee_maker)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
    ]
}
