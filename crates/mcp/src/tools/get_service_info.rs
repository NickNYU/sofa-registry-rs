use reqwest::Client;
use serde_json::Value;
use tracing::debug;

use crate::server::McpConfig;

/// Get detailed information about a specific service by data ID.
pub async fn execute(
    client: &Client,
    config: &McpConfig,
    args: Value,
) -> Result<String, String> {
    let data_id = args
        .get("data_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: data_id".to_string())?;

    // Query publishers for this data ID
    let pub_url = format!(
        "{}/api/data/datum/query?dataId={}",
        config.data_http_url, data_id
    );
    debug!("Querying publishers: {}", pub_url);

    let pub_resp = client.get(&pub_url).send().await;

    // Query subscribers for this data ID from session server
    let sub_url = format!(
        "{}/api/session/subscribers/query?dataId={}",
        config.session_http_url, data_id
    );
    debug!("Querying subscribers: {}", sub_url);

    let sub_resp = client.get(&sub_url).send().await;

    let mut output = format!("Service info for: {}\n\n", data_id);

    // Format publisher info
    output.push_str("== Publishers ==\n");
    match pub_resp {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if status.is_success() {
                match serde_json::from_str::<Value>(&body) {
                    Ok(json) => {
                        output.push_str(
                            &serde_json::to_string_pretty(&json).unwrap_or(body),
                        );
                    }
                    Err(_) => output.push_str(&body),
                }
            } else {
                output.push_str(&format!("Data server returned HTTP {}\n", status));
            }
        }
        Err(e) => {
            output.push_str(&format!(
                "Could not reach data server: {}\n",
                e
            ));
        }
    }

    output.push_str("\n\n== Subscribers ==\n");
    match sub_resp {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if status.is_success() {
                match serde_json::from_str::<Value>(&body) {
                    Ok(json) => {
                        output.push_str(
                            &serde_json::to_string_pretty(&json).unwrap_or(body),
                        );
                    }
                    Err(_) => output.push_str(&body),
                }
            } else {
                output.push_str(&format!("Session server returned HTTP {}\n", status));
            }
        }
        Err(e) => {
            output.push_str(&format!(
                "Could not reach session server: {}\n",
                e
            ));
        }
    }

    Ok(output)
}
