use reqwest::Client;
use serde_json::Value;
use tracing::debug;

use crate::server::McpConfig;

/// List all registered services by querying the data server's datum count endpoint.
pub async fn execute(
    client: &Client,
    config: &McpConfig,
    args: Value,
) -> Result<String, String> {
    let _data_center = args
        .get("data_center")
        .and_then(|v| v.as_str())
        .unwrap_or("default");

    // Query the data server for datum counts
    let url = format!("{}/api/data/datum/count", config.data_http_url);
    debug!("Querying: {}", url);

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to data server: {}", e))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    if !status.is_success() {
        return Ok(format!(
            "Data server returned HTTP {}. The registry may not be running.\nResponse: {}",
            status, body
        ));
    }

    // Try to parse as JSON for pretty formatting
    match serde_json::from_str::<Value>(&body) {
        Ok(json) => {
            let mut output = String::from("Registered services:\n\n");

            if let Some(obj) = json.as_object() {
                for (key, value) in obj {
                    output.push_str(&format!("  {} => {}\n", key, value));
                }
                output.push_str(&format!("\nTotal entries: {}\n", obj.len()));
            } else {
                output.push_str(&serde_json::to_string_pretty(&json).unwrap_or(body));
            }

            Ok(output)
        }
        Err(_) => Ok(format!("Registered services:\n\n{}", body)),
    }
}
