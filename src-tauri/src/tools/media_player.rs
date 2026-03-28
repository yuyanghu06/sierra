use serde_json::json;

use super::{ToolDefinition, ToolParameters};

pub fn tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "media_player_play_media".to_string(),
            description: "Play media on a media player. Use this when the user wants to play \
                a specific song, playlist, radio station, or other media content. Requires \
                the media content ID (like a URL or identifier) and the content type."
                .to_string(),
            domain: "media_player".to_string(),
            service: "play_media".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the media player (e.g. media_player.living_room_speaker)"
                    },
                    "media_content_id": {
                        "type": "string",
                        "description": "The ID of the media content to play (e.g. a URL, playlist ID, or station name)"
                    },
                    "media_content_type": {
                        "type": "string",
                        "description": "The type of media content (e.g. 'music', 'playlist', 'channel', 'tvshow', 'video', 'image')"
                    }
                }),
                required: vec![
                    "entity_id".to_string(),
                    "media_content_id".to_string(),
                    "media_content_type".to_string(),
                ],
            },
        },
        ToolDefinition {
            name: "media_player_media_pause".to_string(),
            description: "Pause the currently playing media. Use this when the user wants to \
                pause music, a video, or any other media playback."
                .to_string(),
            domain: "media_player".to_string(),
            service: "media_pause".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the media player (e.g. media_player.living_room_speaker)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "media_player_media_play".to_string(),
            description: "Resume playing media that was paused. Use this when the user wants to \
                continue or resume playback."
                .to_string(),
            domain: "media_player".to_string(),
            service: "media_play".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the media player (e.g. media_player.living_room_speaker)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "media_player_media_stop".to_string(),
            description: "Stop media playback entirely. Use this when the user wants to stop \
                (not just pause) the media player."
                .to_string(),
            domain: "media_player".to_string(),
            service: "media_stop".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the media player (e.g. media_player.living_room_speaker)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "media_player_volume_set".to_string(),
            description: "Set the volume level on a media player. Use this when the user \
                specifies a volume level or percentage."
                .to_string(),
            domain: "media_player".to_string(),
            service: "volume_set".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the media player (e.g. media_player.living_room_speaker)"
                    },
                    "volume_level": {
                        "type": "number",
                        "description": "Volume level from 0.0 (muted) to 1.0 (maximum). For example, 50% volume is 0.5",
                        "minimum": 0.0,
                        "maximum": 1.0
                    }
                }),
                required: vec!["entity_id".to_string(), "volume_level".to_string()],
            },
        },
        ToolDefinition {
            name: "media_player_volume_up".to_string(),
            description: "Increase the volume on a media player by one step. Use this when the \
                user says 'turn it up' or 'louder' without specifying an exact level."
                .to_string(),
            domain: "media_player".to_string(),
            service: "volume_up".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the media player (e.g. media_player.living_room_speaker)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "media_player_volume_down".to_string(),
            description: "Decrease the volume on a media player by one step. Use this when the \
                user says 'turn it down' or 'quieter' without specifying an exact level."
                .to_string(),
            domain: "media_player".to_string(),
            service: "volume_down".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the media player (e.g. media_player.living_room_speaker)"
                    }
                }),
                required: vec!["entity_id".to_string()],
            },
        },
        ToolDefinition {
            name: "media_player_select_source".to_string(),
            description: "Select an input source on a media player. Use this when the user wants \
                to switch inputs (e.g. 'switch to Bluetooth', 'use the TV input', 'change to \
                Spotify')."
                .to_string(),
            domain: "media_player".to_string(),
            service: "select_source".to_string(),
            parameters: ToolParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "entity_id": {
                        "type": "string",
                        "description": "The entity ID of the media player (e.g. media_player.living_room_speaker)"
                    },
                    "source": {
                        "type": "string",
                        "description": "The source/input to select (e.g. 'Bluetooth', 'HDMI 1', 'Spotify'). Available sources depend on the device"
                    }
                }),
                required: vec!["entity_id".to_string(), "source".to_string()],
            },
        },
    ]
}
