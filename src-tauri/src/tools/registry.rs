use super::ToolDefinition;
use super::{climate, light, media_player, switch};

pub fn get_all_tools() -> Vec<ToolDefinition> {
    let mut tools = Vec::new();
    tools.extend(light::tools());
    tools.extend(switch::tools());
    tools.extend(climate::tools());
    tools.extend(media_player::tools());
    tools
}

pub fn find_tool(name: &str) -> Option<ToolDefinition> {
    get_all_tools().into_iter().find(|t| t.name == name)
}

/// Convert tool definitions into the format Ollama expects in the `tools` field
/// of its /api/chat request.
pub fn tools_for_ollama() -> Vec<serde_json::Value> {
    get_all_tools()
        .into_iter()
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": {
                        "type": tool.parameters.r#type,
                        "properties": tool.parameters.properties,
                        "required": tool.parameters.required,
                    }
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_TOOL_NAMES: &[&str] = &[
        // Light (3)
        "light_turn_on",
        "light_turn_off",
        "light_toggle",
        // Switch (3)
        "switch_turn_on",
        "switch_turn_off",
        "switch_toggle",
        // Climate (5)
        "climate_set_temperature",
        "climate_set_hvac_mode",
        "climate_set_fan_mode",
        "climate_turn_on",
        "climate_turn_off",
        // Media player (8)
        "media_player_play_media",
        "media_player_media_pause",
        "media_player_media_play",
        "media_player_media_stop",
        "media_player_volume_set",
        "media_player_volume_up",
        "media_player_volume_down",
        "media_player_select_source",
    ];

    #[test]
    fn total_tool_count() {
        assert_eq!(get_all_tools().len(), 19);
    }

    #[test]
    fn all_expected_tools_exist() {
        let tools = get_all_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        for expected in ALL_TOOL_NAMES {
            assert!(
                names.contains(expected),
                "Missing tool: {}",
                expected
            );
        }
    }

    #[test]
    fn no_duplicate_tool_names() {
        let tools = get_all_tools();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "Duplicate tool names found");
    }

    #[test]
    fn find_tool_returns_correct_tool() {
        for name in ALL_TOOL_NAMES {
            let tool = find_tool(name);
            assert!(tool.is_some(), "find_tool returned None for {}", name);
            assert_eq!(tool.unwrap().name, *name);
        }
    }

    #[test]
    fn find_tool_returns_none_for_unknown() {
        assert!(find_tool("nonexistent").is_none());
        assert!(find_tool("").is_none());
        assert!(find_tool("light_dim").is_none());
    }

    // ──────────────────────────────────────────────
    // Domain counts
    // ──────────────────────────────────────────────

    #[test]
    fn light_domain_has_3_tools() {
        assert_eq!(light::tools().len(), 3);
    }

    #[test]
    fn switch_domain_has_3_tools() {
        assert_eq!(switch::tools().len(), 3);
    }

    #[test]
    fn climate_domain_has_5_tools() {
        assert_eq!(climate::tools().len(), 5);
    }

    #[test]
    fn media_player_domain_has_8_tools() {
        assert_eq!(media_player::tools().len(), 8);
    }

    // ──────────────────────────────────────────────
    // Domain and service fields
    // ──────────────────────────────────────────────

    #[test]
    fn light_tools_have_correct_domain() {
        for tool in light::tools() {
            assert_eq!(tool.domain, "light", "Tool {} has wrong domain", tool.name);
        }
    }

    #[test]
    fn switch_tools_have_correct_domain() {
        for tool in switch::tools() {
            assert_eq!(tool.domain, "switch", "Tool {} has wrong domain", tool.name);
        }
    }

    #[test]
    fn climate_tools_have_correct_domain() {
        for tool in climate::tools() {
            assert_eq!(
                tool.domain, "climate",
                "Tool {} has wrong domain",
                tool.name
            );
        }
    }

    #[test]
    fn media_player_tools_have_correct_domain() {
        for tool in media_player::tools() {
            assert_eq!(
                tool.domain, "media_player",
                "Tool {} has wrong domain",
                tool.name
            );
        }
    }

    #[test]
    fn tool_names_match_domain_service_pattern() {
        for tool in get_all_tools() {
            let expected = format!("{}_{}", tool.domain, tool.service);
            assert_eq!(
                tool.name, expected,
                "Tool name '{}' does not match domain '{}' + service '{}'",
                tool.name, tool.domain, tool.service
            );
        }
    }

    // ──────────────────────────────────────────────
    // Parameter validation
    // ──────────────────────────────────────────────

    #[test]
    fn all_tools_have_object_type_params() {
        for tool in get_all_tools() {
            assert_eq!(
                tool.parameters.r#type, "object",
                "Tool {} parameters.type is not 'object'",
                tool.name
            );
        }
    }

    #[test]
    fn all_tools_require_entity_id() {
        for tool in get_all_tools() {
            assert!(
                tool.parameters.required.contains(&"entity_id".to_string()),
                "Tool {} does not require entity_id",
                tool.name
            );
        }
    }

    #[test]
    fn all_tools_define_entity_id_property() {
        for tool in get_all_tools() {
            assert!(
                tool.parameters.properties.get("entity_id").is_some(),
                "Tool {} does not define entity_id property",
                tool.name
            );
        }
    }

    #[test]
    fn all_tools_have_descriptions() {
        for tool in get_all_tools() {
            assert!(
                !tool.description.is_empty(),
                "Tool {} has empty description",
                tool.name
            );
        }
    }

    // ──────────────────────────────────────────────
    // Specific parameter schemas
    // ──────────────────────────────────────────────

    #[test]
    fn light_turn_on_has_optional_params() {
        let tool = find_tool("light_turn_on").unwrap();
        let props = &tool.parameters.properties;
        assert!(props.get("brightness").is_some());
        assert!(props.get("color_temp").is_some());
        assert!(props.get("rgb_color").is_some());
        assert!(props.get("transition").is_some());
        assert!(props.get("effect").is_some());
        // Only entity_id is required
        assert_eq!(tool.parameters.required, vec!["entity_id"]);
    }

    #[test]
    fn light_turn_off_has_transition_param() {
        let tool = find_tool("light_turn_off").unwrap();
        assert!(tool.parameters.properties.get("transition").is_some());
    }

    #[test]
    fn climate_set_temperature_has_temp_params() {
        let tool = find_tool("climate_set_temperature").unwrap();
        let props = &tool.parameters.properties;
        assert!(props.get("temperature").is_some());
        assert!(props.get("target_temp_high").is_some());
        assert!(props.get("target_temp_low").is_some());
    }

    #[test]
    fn climate_set_hvac_mode_requires_hvac_mode() {
        let tool = find_tool("climate_set_hvac_mode").unwrap();
        assert!(tool.parameters.required.contains(&"hvac_mode".to_string()));
        let hvac_enum = &tool.parameters.properties["hvac_mode"]["enum"];
        let modes: Vec<&str> = hvac_enum
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(modes.contains(&"heat"));
        assert!(modes.contains(&"cool"));
        assert!(modes.contains(&"auto"));
        assert!(modes.contains(&"off"));
        assert!(modes.contains(&"heat_cool"));
        assert!(modes.contains(&"fan_only"));
        assert!(modes.contains(&"dry"));
    }

    #[test]
    fn climate_set_fan_mode_requires_fan_mode() {
        let tool = find_tool("climate_set_fan_mode").unwrap();
        assert!(tool.parameters.required.contains(&"fan_mode".to_string()));
        let fan_enum = &tool.parameters.properties["fan_mode"]["enum"];
        let modes: Vec<&str> = fan_enum
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(modes.contains(&"auto"));
        assert!(modes.contains(&"low"));
        assert!(modes.contains(&"medium"));
        assert!(modes.contains(&"high"));
        assert!(modes.contains(&"off"));
    }

    #[test]
    fn media_player_play_media_requires_content_params() {
        let tool = find_tool("media_player_play_media").unwrap();
        assert!(tool
            .parameters
            .required
            .contains(&"media_content_id".to_string()));
        assert!(tool
            .parameters
            .required
            .contains(&"media_content_type".to_string()));
    }

    #[test]
    fn media_player_volume_set_requires_volume_level() {
        let tool = find_tool("media_player_volume_set").unwrap();
        assert!(tool
            .parameters
            .required
            .contains(&"volume_level".to_string()));
        let vol = &tool.parameters.properties["volume_level"];
        assert_eq!(vol["minimum"], 0.0);
        assert_eq!(vol["maximum"], 1.0);
    }

    #[test]
    fn media_player_select_source_requires_source() {
        let tool = find_tool("media_player_select_source").unwrap();
        assert!(tool.parameters.required.contains(&"source".to_string()));
    }

    // ──────────────────────────────────────────────
    // Ollama format
    // ──────────────────────────────────────────────

    #[test]
    fn tools_for_ollama_count() {
        assert_eq!(tools_for_ollama().len(), 19);
    }

    #[test]
    fn tools_for_ollama_format() {
        for tool_json in tools_for_ollama() {
            assert_eq!(tool_json["type"], "function");
            let func = &tool_json["function"];
            assert!(func["name"].is_string());
            assert!(func["description"].is_string());
            assert!(func["parameters"].is_object());
            assert_eq!(func["parameters"]["type"], "object");
            assert!(func["parameters"]["properties"].is_object());
            assert!(func["parameters"]["required"].is_array());
        }
    }

    #[test]
    fn tools_for_ollama_contains_all_names() {
        let ollama_tools = tools_for_ollama();
        let names: Vec<&str> = ollama_tools
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        for expected in ALL_TOOL_NAMES {
            assert!(
                names.contains(expected),
                "Ollama format missing tool: {}",
                expected
            );
        }
    }
}
