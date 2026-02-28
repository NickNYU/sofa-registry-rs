use std::io::{self, BufRead, Write};
use tracing::{debug, error, info};

use crate::server::{JsonRpcRequest, JsonRpcResponse, McpServer};

/// Run the MCP server reading from stdin and writing to stdout.
///
/// This is the standard MCP transport for CLI-based integrations.
/// Each line on stdin is expected to be a complete JSON-RPC request.
/// Each response is written as a single JSON line to stdout.
pub async fn run(server: &McpServer) -> Result<(), Box<dyn std::error::Error>> {
    info!("MCP stdio transport starting");

    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        debug!("stdin <<< {}", line);

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse JSON-RPC request: {}", e);
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: None,
                    result: None,
                    error: Some(crate::server::JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                };
                let serialized = serde_json::to_string(&resp)?;
                let mut out = stdout.lock();
                writeln!(out, "{}", serialized)?;
                out.flush()?;
                continue;
            }
        };

        let response = server.handle_request(request).await;
        let serialized = serde_json::to_string(&response)?;

        debug!("stdout >>> {}", serialized);

        let mut out = stdout.lock();
        writeln!(out, "{}", serialized)?;
        out.flush()?;
    }

    info!("MCP stdio transport shutting down");
    Ok(())
}
