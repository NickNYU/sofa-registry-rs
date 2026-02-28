use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, info};

use crate::tools;

/// MCP JSON-RPC request.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// MCP JSON-RPC response.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

/// MCP tool definition exposed to AI assistants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Configuration for connecting to SOFARegistry HTTP admin APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Meta server HTTP endpoint (e.g. "http://127.0.0.1:9612").
    pub meta_http_url: String,
    /// Session server HTTP endpoint (e.g. "http://127.0.0.1:9602").
    pub session_http_url: String,
    /// Data server HTTP endpoint (e.g. "http://127.0.0.1:9622").
    pub data_http_url: String,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            meta_http_url: "http://127.0.0.1:9612".into(),
            session_http_url: "http://127.0.0.1:9602".into(),
            data_http_url: "http://127.0.0.1:9622".into(),
        }
    }
}

/// The MCP server that exposes SOFARegistry tools via JSON-RPC.
pub struct McpServer {
    config: McpConfig,
    http_client: reqwest::Client,
    tools: Vec<ToolDefinition>,
}

impl McpServer {
    /// Create a new MCP server with the given configuration.
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(config: McpConfig) -> std::result::Result<Self, reqwest::Error> {
        let tools = tools::all_tool_definitions();
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self {
            config,
            http_client,
            tools,
        })
    }

    /// Run the MCP server over stdio, reading JSON-RPC requests line-by-line
    /// from stdin and writing responses to stdout.
    pub async fn run_stdio(&self) -> Result<(), Box<dyn std::error::Error>> {
        use std::io::{self, BufRead, Write};

        info!("MCP server starting on stdio");

        let stdin = io::stdin();
        let stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            debug!("Received: {}", line);

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {}", e),
                        }),
                    };
                    let mut out = stdout.lock();
                    writeln!(out, "{}", serde_json::to_string(&resp)?)?;
                    out.flush()?;
                    continue;
                }
            };

            let response = self.handle_request(request).await;
            let serialized = serde_json::to_string(&response)?;
            debug!("Sending: {}", serialized);

            let mut out = stdout.lock();
            writeln!(out, "{}", serialized)?;
            out.flush()?;
        }

        info!("MCP server shutting down");
        Ok(())
    }

    /// Handle a single JSON-RPC request and return a response.
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!("Handling method: {}", request.method);

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "initialized" => self.handle_notification(request.id),
            "tools/list" => self.handle_list_tools(request.id),
            "tools/call" => self.handle_tool_call(request.id, request.params).await,
            "ping" => self.handle_ping(request.id),
            _ => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", ""),
                }),
            },
        }
    }

    fn handle_initialize(&self, id: Option<Value>) -> JsonRpcResponse {
        info!("MCP initialize request received");
        JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "sofa-registry-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            error: None,
        }
    }

    fn handle_notification(&self, _id: Option<Value>) -> JsonRpcResponse {
        // Notifications do not require a response in MCP, but since our
        // loop always writes a response we return an empty result.
        JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: None,
            result: Some(Value::Null),
            error: None,
        }
    }

    fn handle_ping(&self, id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(serde_json::json!({})),
            error: None,
        }
    }

    fn handle_list_tools(&self, id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(serde_json::json!({ "tools": self.tools })),
            error: None,
        }
    }

    async fn handle_tool_call(&self, id: Option<Value>, params: Value) -> JsonRpcResponse {
        let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));

        info!("Tool call: {} with args: {}", tool_name, args);

        let result = match tool_name {
            "list_services" => {
                tools::list_services::execute(&self.http_client, &self.config, args).await
            }
            "get_service_info" => {
                tools::get_service_info::execute(&self.http_client, &self.config, args).await
            }
            "search_services" => {
                tools::search_services::execute(&self.http_client, &self.config, args).await
            }
            "get_cluster_health" => {
                tools::get_cluster_health::execute(&self.http_client, &self.config, args).await
            }
            "get_slot_table" => {
                tools::get_slot_table::execute(&self.http_client, &self.config, args).await
            }
            _ => Err(format!("Unknown tool: {}", tool_name)),
        };

        match result {
            Ok(content) => JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id,
                result: Some(serde_json::json!({
                    "content": [{"type": "text", "text": content}]
                })),
                error: None,
            },
            Err(e) => {
                error!("Tool {} failed: {}", tool_name, e);
                JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id,
                    result: Some(serde_json::json!({
                        "content": [{"type": "text", "text": format!("Error: {}", e)}],
                        "isError": true
                    })),
                    error: None,
                }
            }
        }
    }

    /// Returns a reference to the server configuration.
    pub fn config(&self) -> &McpConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = McpConfig::default();
        assert_eq!(cfg.meta_http_url, "http://127.0.0.1:9612");
        assert_eq!(cfg.session_http_url, "http://127.0.0.1:9602");
        assert_eq!(cfg.data_http_url, "http://127.0.0.1:9622");
    }

    #[tokio::test]
    async fn test_handle_initialize() {
        let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::Number(1.into())),
            method: "initialize".into(),
            params: Value::Object(Default::default()),
        };
        let resp = server.handle_request(req).await;
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "sofa-registry-mcp");
    }

    #[tokio::test]
    async fn test_handle_list_tools() {
        let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::Number(2.into())),
            method: "tools/list".into(),
            params: Value::Object(Default::default()),
        };
        let resp = server.handle_request(req).await;
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 5);
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::Number(3.into())),
            method: "unknown/method".into(),
            params: Value::Object(Default::default()),
        };
        let resp = server.handle_request(req).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_handle_unknown_tool() {
        let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::Number(4.into())),
            method: "tools/call".into(),
            params: serde_json::json!({"name": "nonexistent_tool"}),
        };
        let resp = server.handle_request(req).await;
        assert!(resp.error.is_none()); // MCP returns tool errors in result
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
    }

    #[tokio::test]
    async fn test_handle_ping() {
        let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::Number(5.into())),
            method: "ping".into(),
            params: Value::Object(Default::default()),
        };
        let resp = server.handle_request(req).await;
        assert!(resp.error.is_none());
    }
}
