pub mod get_cluster_health;
pub mod get_service_info;
pub mod get_slot_table;
pub mod list_services;
pub mod search_services;

use crate::server::ToolDefinition;

/// Returns definitions for all MCP tools exposed by this server.
pub fn all_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "list_services".into(),
            description: "List all registered services in the SOFARegistry. Returns service \
                          data IDs and publisher/subscriber counts."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "data_center": {
                        "type": "string",
                        "description": "Data center to query (optional, defaults to all)"
                    }
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "get_service_info".into(),
            description: "Get detailed information about a specific service including its \
                          publishers, subscribers, and data versions."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "data_id": {
                        "type": "string",
                        "description": "The service data ID to look up"
                    }
                },
                "required": ["data_id"]
            }),
        },
        ToolDefinition {
            name: "search_services".into(),
            description: "Search for services by name pattern (substring match). Returns \
                          matching service data IDs."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Search pattern to match against service data IDs"
                    }
                },
                "required": ["pattern"]
            }),
        },
        ToolDefinition {
            name: "get_cluster_health".into(),
            description: "Get the health status of all SOFARegistry cluster components \
                          including meta, data, and session servers."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "get_slot_table".into(),
            description: "Get the current slot table showing how data is partitioned across \
                          data servers. Shows slot assignments and epochs."
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}
