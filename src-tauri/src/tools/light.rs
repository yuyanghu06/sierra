use serde_json::json;

use super::{ToolDefinition, ToolParameters};

pub fn tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "light_turn_on".to_string(),
            description: "Turn on a light. Use this when the user wants to turn on, brighten, \
                or change the color of a light. The entity_id identifies which light. Optionally \
                set brightness (0-255, where 255 is full brightness), color temperature in mireds, \
                RGB color as [r, g, b] where each value is 0-255, transition time in seconds, \
                or a named effect."
                .to_string(),
            domain: "light".to_string(),
            service: "turn_on".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the light (e.g. light.living_room)"
                    },
                    "brightness": {
                        "type": "integer",
                        "description": "Brightness level, 0-255 where 255 is full brightness",
                        "minimum": 0,
                        "maximum": 255
                    },
                    "color_temp": {
                        "type": "integer",
                        "description": "Color temperature in mireds. Lower values are cooler (daylight), higher values are warmer (candlelight). Typical range: 153-500"
                    },
                    "rgb_color": {
                        "type": "array",
                        "description": "RGB color as [red, green, blue], each 0-255",
                        "items": { "type": "integer" },
                        "minItems": 3,
                        "maxItems": 3
                    },
                    "transition": {
                        "type": "number",
                        "description": "Transition duration in seconds for the light to change to the new state"
                    },
                    "effect": {
                        "type": "string",
                        "description": "Named light effect (e.g. 'colorloop', 'random'). Available effects depend on the light hardware"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "light_turn_off".to_string(),
            description: "Turn off a light. Use this when the user wants to turn off or shut down \
                a light."
                .to_string(),
            domain: "light".to_string(),
            service: "turn_off".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the light (e.g. light.living_room)"
                    },
                    "transition": {
                        "type": "number",
                        "description": "Transition duration in seconds for the light to turn off"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "light_toggle".to_string(),
            description: "Toggle a light between on and off. Use this when the user wants to \
                switch a light to the opposite state without specifying on or off explicitly."
                .to_string(),
            domain: "light".to_string(),
            service: "toggle".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the light (e.g. light.living_room)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
    ]
}
