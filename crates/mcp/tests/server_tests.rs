use serde_json::Value;
use sofa_registry_mcp::server::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpConfig, ToolDefinition};
use sofa_registry_mcp::McpServer;

// ---------------------------------------------------------------------------
// JsonRpcRequest serialization / deserialization
// ---------------------------------------------------------------------------

#[test]
fn jsonrpc_request_deserializes_from_json() {
    let json = r#"{
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }"#;
    let req: JsonRpcRequest = serde_json::from_str(json).expect("should parse");
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, Some(Value::Number(1.into())));
    assert_eq!(req.method, "initialize");
    assert!(req.params.is_object());
}

#[test]
fn jsonrpc_request_with_null_id() {
    let json = r#"{
        "jsonrpc": "2.0",
        "id": null,
        "method": "ping",
        "params": {}
    }"#;
    let req: JsonRpcRequest = serde_json::from_str(json).expect("should parse");
    // serde deserializes `"id": null` as `None` for `Option<Value>`
    assert!(req.id.is_none());
}

#[test]
fn jsonrpc_request_without_id() {
    let json = r#"{
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    }"#;
    let req: JsonRpcRequest = serde_json::from_str(json).expect("should parse");
    assert!(req.id.is_none());
}

#[test]
fn jsonrpc_request_with_string_id() {
    let json = r#"{
        "jsonrpc": "2.0",
        "id": "abc-123",
        "method": "tools/list",
        "params": {}
    }"#;
    let req: JsonRpcRequest = serde_json::from_str(json).expect("should parse");
    assert_eq!(req.id, Some(Value::String("abc-123".into())));
}

#[test]
fn jsonrpc_request_params_default_to_null_when_missing() {
    let json = r#"{
        "jsonrpc": "2.0",
        "id": 1,
        "method": "ping"
    }"#;
    let req: JsonRpcRequest = serde_json::from_str(json).expect("should parse");
    // params has `#[serde(default)]` so it defaults to Value::Null
    assert!(req.params.is_null());
}

#[test]
fn jsonrpc_request_with_complex_params() {
    let json = r#"{
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/call",
        "params": {
            "name": "get_service_info",
            "arguments": {"data_id": "com.example.Svc"}
        }
    }"#;
    let req: JsonRpcRequest = serde_json::from_str(json).expect("should parse");
    assert_eq!(req.method, "tools/call");
    assert_eq!(
        req.params.get("name").and_then(|v| v.as_str()),
        Some("get_service_info")
    );
    assert_eq!(
        req.params
            .get("arguments")
            .and_then(|a| a.get("data_id"))
            .and_then(|v| v.as_str()),
        Some("com.example.Svc")
    );
}

#[test]
fn jsonrpc_request_serializes_to_json() {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(10.into())),
        method: "tools/list".into(),
        params: Value::Object(Default::default()),
    };
    let json = serde_json::to_string(&req).expect("should serialize");
    assert!(json.contains("\"jsonrpc\":\"2.0\""));
    assert!(json.contains("\"method\":\"tools/list\""));
}

#[test]
fn jsonrpc_request_roundtrip() {
    let original = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(99.into())),
        method: "ping".into(),
        params: serde_json::json!({"key": "value"}),
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: JsonRpcRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.jsonrpc, original.jsonrpc);
    assert_eq!(restored.id, original.id);
    assert_eq!(restored.method, original.method);
    assert_eq!(restored.params, original.params);
}

// ---------------------------------------------------------------------------
// JsonRpcResponse serialization / deserialization
// ---------------------------------------------------------------------------

#[test]
fn jsonrpc_response_success_serializes_correctly() {
    let resp = JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(1.into())),
        result: Some(serde_json::json!({"tools": []})),
        error: None,
    };
    let json = serde_json::to_string(&resp).expect("should serialize");
    assert!(json.contains("\"jsonrpc\":\"2.0\""));
    assert!(json.contains("\"result\""));
    // error should be omitted via skip_serializing_if
    assert!(!json.contains("\"error\""));
}

#[test]
fn jsonrpc_response_error_serializes_correctly() {
    let resp = JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(2.into())),
        result: None,
        error: Some(JsonRpcError {
            code: -32601,
            message: "Method not found".into(),
        }),
    };
    let json = serde_json::to_string(&resp).expect("should serialize");
    assert!(json.contains("\"error\""));
    assert!(json.contains("-32601"));
    assert!(json.contains("Method not found"));
    // result should be omitted via skip_serializing_if
    assert!(!json.contains("\"result\""));
}

#[test]
fn jsonrpc_response_deserializes_success() {
    let json = r#"{
        "jsonrpc": "2.0",
        "id": 1,
        "result": {"tools": []}
    }"#;
    let resp: JsonRpcResponse = serde_json::from_str(json).expect("should parse");
    assert_eq!(resp.jsonrpc, "2.0");
    assert!(resp.result.is_some());
    assert!(resp.error.is_none());
}

#[test]
fn jsonrpc_response_deserializes_error() {
    let json = r#"{
        "jsonrpc": "2.0",
        "id": 2,
        "error": {"code": -32700, "message": "Parse error"}
    }"#;
    let resp: JsonRpcResponse = serde_json::from_str(json).expect("should parse");
    assert!(resp.result.is_none());
    let err = resp.error.unwrap();
    assert_eq!(err.code, -32700);
    assert_eq!(err.message, "Parse error");
}

#[test]
fn jsonrpc_response_roundtrip_success() {
    let original = JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(5.into())),
        result: Some(serde_json::json!({"data": "test"})),
        error: None,
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: JsonRpcResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.jsonrpc, original.jsonrpc);
    assert_eq!(restored.id, original.id);
    assert_eq!(restored.result, original.result);
    assert!(restored.error.is_none());
}

#[test]
fn jsonrpc_response_roundtrip_error() {
    let original = JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(6.into())),
        result: None,
        error: Some(JsonRpcError {
            code: -32600,
            message: "Invalid request".into(),
        }),
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: JsonRpcResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.jsonrpc, original.jsonrpc);
    assert_eq!(restored.id, original.id);
    assert!(restored.result.is_none());
    let err = restored.error.unwrap();
    assert_eq!(err.code, -32600);
    assert_eq!(err.message, "Invalid request");
}

// ---------------------------------------------------------------------------
// JsonRpcError
// ---------------------------------------------------------------------------

#[test]
fn jsonrpc_error_serializes() {
    let err = JsonRpcError {
        code: -32700,
        message: "Parse error".into(),
    };
    let json = serde_json::to_string(&err).expect("should serialize");
    assert!(json.contains("-32700"));
    assert!(json.contains("Parse error"));
}

#[test]
fn jsonrpc_error_deserializes() {
    let json = r#"{"code": -32601, "message": "Method not found"}"#;
    let err: JsonRpcError = serde_json::from_str(json).expect("should parse");
    assert_eq!(err.code, -32601);
    assert_eq!(err.message, "Method not found");
}

// ---------------------------------------------------------------------------
// ToolDefinition serde
// ---------------------------------------------------------------------------

#[test]
fn tool_definition_serializes_with_camel_case_input_schema() {
    let def = ToolDefinition {
        name: "test_tool".into(),
        description: "A test tool".into(),
        input_schema: serde_json::json!({"type": "object", "properties": {}}),
    };
    let json = serde_json::to_string(&def).expect("should serialize");
    assert!(json.contains("\"inputSchema\""));
    assert!(!json.contains("\"input_schema\""));
}

#[test]
fn tool_definition_deserializes_from_camel_case() {
    let json = r#"{
        "name": "my_tool",
        "description": "Does things",
        "inputSchema": {"type": "object", "properties": {}, "required": []}
    }"#;
    let def: ToolDefinition = serde_json::from_str(json).expect("should parse");
    assert_eq!(def.name, "my_tool");
    assert_eq!(def.description, "Does things");
    assert_eq!(
        def.input_schema.get("type").and_then(|v| v.as_str()),
        Some("object")
    );
}

// ---------------------------------------------------------------------------
// McpConfig
// ---------------------------------------------------------------------------

#[test]
fn mcp_config_default_values() {
    let cfg = McpConfig::default();
    assert_eq!(cfg.meta_http_url, "http://127.0.0.1:9612");
    assert_eq!(cfg.session_http_url, "http://127.0.0.1:9602");
    assert_eq!(cfg.data_http_url, "http://127.0.0.1:9622");
}

#[test]
fn mcp_config_custom_values() {
    let cfg = McpConfig {
        meta_http_url: "http://meta:9612".into(),
        session_http_url: "http://session:9602".into(),
        data_http_url: "http://data:9622".into(),
    };
    assert_eq!(cfg.meta_http_url, "http://meta:9612");
    assert_eq!(cfg.session_http_url, "http://session:9602");
    assert_eq!(cfg.data_http_url, "http://data:9622");
}

#[test]
fn mcp_config_clone() {
    let cfg1 = McpConfig::default();
    let cfg2 = cfg1.clone();
    assert_eq!(cfg1.meta_http_url, cfg2.meta_http_url);
    assert_eq!(cfg1.session_http_url, cfg2.session_http_url);
    assert_eq!(cfg1.data_http_url, cfg2.data_http_url);
}

#[test]
fn mcp_config_debug() {
    let cfg = McpConfig::default();
    let dbg = format!("{:?}", cfg);
    assert!(dbg.contains("McpConfig"));
}

#[test]
fn mcp_config_serializes_to_json() {
    let cfg = McpConfig::default();
    let json = serde_json::to_string(&cfg).expect("should serialize");
    assert!(json.contains("http://127.0.0.1:9612"));
    assert!(json.contains("http://127.0.0.1:9602"));
    assert!(json.contains("http://127.0.0.1:9622"));
}

#[test]
fn mcp_config_deserializes_from_json() {
    let json = r#"{
        "meta_http_url": "http://m:1",
        "session_http_url": "http://s:2",
        "data_http_url": "http://d:3"
    }"#;
    let cfg: McpConfig = serde_json::from_str(json).expect("should parse");
    assert_eq!(cfg.meta_http_url, "http://m:1");
    assert_eq!(cfg.session_http_url, "http://s:2");
    assert_eq!(cfg.data_http_url, "http://d:3");
}

// ---------------------------------------------------------------------------
// McpServer handle_request tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn handle_initialize_returns_protocol_version() {
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
}

#[tokio::test]
async fn handle_initialize_returns_server_info() {
    let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(1.into())),
        method: "initialize".into(),
        params: Value::Object(Default::default()),
    };
    let resp = server.handle_request(req).await;
    let result = resp.result.unwrap();
    assert_eq!(result["serverInfo"]["name"], "sofa-registry-mcp");
    // version should be present and non-empty
    let version = result["serverInfo"]["version"].as_str().unwrap();
    assert!(!version.is_empty());
}

#[tokio::test]
async fn handle_initialize_returns_capabilities() {
    let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(1.into())),
        method: "initialize".into(),
        params: Value::Object(Default::default()),
    };
    let resp = server.handle_request(req).await;
    let result = resp.result.unwrap();
    assert!(result["capabilities"]["tools"].is_object());
}

#[tokio::test]
async fn handle_initialize_preserves_request_id() {
    let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(42.into())),
        method: "initialize".into(),
        params: Value::Object(Default::default()),
    };
    let resp = server.handle_request(req).await;
    assert_eq!(resp.id, Some(Value::Number(42.into())));
}

#[tokio::test]
async fn handle_list_tools_returns_all_five_tools() {
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
async fn handle_list_tools_returns_expected_tool_names() {
    let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(2.into())),
        method: "tools/list".into(),
        params: Value::Object(Default::default()),
    };
    let resp = server.handle_request(req).await;
    let result = resp.result.unwrap();
    let tools = result["tools"].as_array().unwrap();
    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"list_services"));
    assert!(names.contains(&"get_service_info"));
    assert!(names.contains(&"search_services"));
    assert!(names.contains(&"get_cluster_health"));
    assert!(names.contains(&"get_slot_table"));
}

#[tokio::test]
async fn handle_ping_returns_empty_result() {
    let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(3.into())),
        method: "ping".into(),
        params: Value::Object(Default::default()),
    };
    let resp = server.handle_request(req).await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert!(result.is_object());
}

#[tokio::test]
async fn handle_unknown_method_returns_error() {
    let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(4.into())),
        method: "nonexistent/method".into(),
        params: Value::Object(Default::default()),
    };
    let resp = server.handle_request(req).await;
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, -32601);
}

#[tokio::test]
async fn handle_unknown_tool_returns_error_in_result() {
    let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(Value::Number(5.into())),
        method: "tools/call".into(),
        params: serde_json::json!({"name": "does_not_exist"}),
    };
    let resp = server.handle_request(req).await;
    // MCP returns tool errors in result, not in error field
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["isError"], true);
}

#[tokio::test]
async fn handle_initialized_returns_null_result() {
    let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: None,
        method: "initialized".into(),
        params: Value::Object(Default::default()),
    };
    let resp = server.handle_request(req).await;
    assert!(resp.error.is_none());
    assert_eq!(resp.result, Some(Value::Null));
    assert!(resp.id.is_none());
}

#[tokio::test]
async fn response_jsonrpc_field_is_always_2_0() {
    let server = McpServer::new(McpConfig::default()).expect("failed to build HTTP client");

    // Test across multiple methods
    let methods = vec!["initialize", "ping", "tools/list", "unknown"];
    for method in methods {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::Number(1.into())),
            method: method.into(),
            params: Value::Object(Default::default()),
        };
        let resp = server.handle_request(req).await;
        assert_eq!(
            resp.jsonrpc, "2.0",
            "jsonrpc should be '2.0' for method '{}'",
            method
        );
    }
}

#[tokio::test]
async fn server_config_accessor() {
    let cfg = McpConfig {
        meta_http_url: "http://custom-meta:9612".into(),
        session_http_url: "http://custom-session:9602".into(),
        data_http_url: "http://custom-data:9622".into(),
    };
    let server = McpServer::new(cfg).expect("failed to build HTTP client");
    assert_eq!(server.config().meta_http_url, "http://custom-meta:9612");
    assert_eq!(
        server.config().session_http_url,
        "http://custom-session:9602"
    );
    assert_eq!(server.config().data_http_url, "http://custom-data:9622");
}
