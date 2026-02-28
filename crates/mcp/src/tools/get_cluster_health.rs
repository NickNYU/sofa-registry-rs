use reqwest::Client;
use serde_json::Value;
use tracing::debug;

use crate::server::McpConfig;

/// Get health status of all registry cluster components.
pub async fn execute(client: &Client, config: &McpConfig, _args: Value) -> Result<String, String> {
    let mut output = String::from("SOFARegistry Cluster Health\n\n");

    // Check meta server health
    output.push_str("== Meta Server ==\n");
    check_health(client, &config.meta_http_url, &mut output).await;

    // Check data server health
    output.push_str("\n== Data Server ==\n");
    check_health(client, &config.data_http_url, &mut output).await;

    // Check session server health
    output.push_str("\n== Session Server ==\n");
    check_health(client, &config.session_http_url, &mut output).await;

    // Try to get meta leader info
    output.push_str("\n== Meta Leader ==\n");
    let leader_url = format!("{}/api/meta/leader/query", config.meta_http_url);
    debug!("Querying meta leader: {}", leader_url);
    match client.get(&leader_url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if status.is_success() {
                match serde_json::from_str::<Value>(&body) {
                    Ok(json) => {
                        output.push_str(&format!(
                            "  {}\n",
                            serde_json::to_string_pretty(&json).unwrap_or(body)
                        ));
                    }
                    Err(_) => output.push_str(&format!("  {}\n", body)),
                }
            } else {
                output.push_str(&format!("  HTTP {} - {}\n", status, body));
            }
        }
        Err(e) => {
            output.push_str(&format!("  Unreachable: {}\n", e));
        }
    }

    Ok(output)
}

async fn check_health(client: &Client, base_url: &str, output: &mut String) {
    let url = format!("{}/health/check", base_url);
    debug!("Health check: {}", url);

    match client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if status.is_success() {
                output.push_str(&format!("  Status: HEALTHY ({})\n", base_url));
                if !body.is_empty() && body != "ok" && body != "OK" {
                    output.push_str(&format!("  Details: {}\n", body));
                }
            } else {
                output.push_str(&format!(
                    "  Status: UNHEALTHY (HTTP {}) at {}\n",
                    status, base_url
                ));
                output.push_str(&format!("  Response: {}\n", body));
            }
        }
        Err(e) => {
            output.push_str(&format!(
                "  Status: UNREACHABLE at {}\n  Error: {}\n",
                base_url, e
            ));
        }
    }
}
