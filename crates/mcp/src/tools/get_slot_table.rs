use reqwest::Client;
use serde_json::Value;
use tracing::debug;

use crate::server::McpConfig;

/// Get the current slot table showing data partitioning across servers.
pub async fn execute(client: &Client, config: &McpConfig, _args: Value) -> Result<String, String> {
    let url = format!("{}/api/meta/slot/table/query", config.meta_http_url);
    debug!("Querying slot table: {}", url);

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to meta server: {}", e))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    if !status.is_success() {
        return Ok(format!(
            "Meta server returned HTTP {}. The registry may not be running.\nResponse: {}",
            status, body
        ));
    }

    match serde_json::from_str::<Value>(&body) {
        Ok(json) => {
            let mut output = String::from("Slot Table:\n\n");

            if let Some(obj) = json.as_object() {
                // Try to extract epoch
                if let Some(epoch) = obj.get("epoch") {
                    output.push_str(&format!("  Epoch: {}\n", epoch));
                }

                // Try to extract slot assignments
                if let Some(slots) = obj.get("slots") {
                    if let Some(slot_arr) = slots.as_array() {
                        output.push_str(&format!("  Total slots: {}\n\n", slot_arr.len()));
                        // Show summary of slot distribution
                        let mut server_slots: std::collections::HashMap<String, usize> =
                            std::collections::HashMap::new();
                        for slot in slot_arr {
                            if let Some(leader) = slot.get("leader").and_then(|l| l.as_str()) {
                                *server_slots.entry(leader.to_string()).or_default() += 1;
                            }
                        }
                        output.push_str("  Slot distribution by leader:\n");
                        for (server, count) in &server_slots {
                            output.push_str(&format!("    {} => {} slots\n", server, count));
                        }
                    } else {
                        output.push_str(&format!(
                            "  Slots: {}\n",
                            serde_json::to_string_pretty(slots).unwrap_or_default()
                        ));
                    }
                } else {
                    // Just dump the whole thing if structure is unexpected
                    output.push_str(&serde_json::to_string_pretty(&json).unwrap_or(body));
                }
            } else {
                output.push_str(&serde_json::to_string_pretty(&json).unwrap_or(body));
            }

            Ok(output)
        }
        Err(_) => Ok(format!("Slot table (raw):\n\n{}", body)),
    }
}
