use reqwest::Client;
use serde_json::Value;
use tracing::debug;

use crate::server::McpConfig;

/// Search for services by name pattern (substring match).
pub async fn execute(
    client: &Client,
    config: &McpConfig,
    args: Value,
) -> Result<String, String> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: pattern".to_string())?;

    // First get all services via the datum count endpoint
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

    let pattern_lower = pattern.to_lowercase();

    match serde_json::from_str::<Value>(&body) {
        Ok(json) => {
            let mut output = format!("Search results for pattern \"{}\":\n\n", pattern);
            let mut match_count = 0;

            if let Some(obj) = json.as_object() {
                for (key, value) in obj {
                    if key.to_lowercase().contains(&pattern_lower) {
                        output.push_str(&format!("  {} => {}\n", key, value));
                        match_count += 1;
                    }
                }
            }

            if match_count == 0 {
                output.push_str("  (no matching services found)\n");
            } else {
                output.push_str(&format!("\n{} service(s) matched.\n", match_count));
            }

            Ok(output)
        }
        Err(_) => Ok(format!(
            "Search results for \"{}\" (raw):\n\n{}",
            pattern, body
        )),
    }
}
