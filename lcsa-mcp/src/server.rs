//! MCP Server Implementation
//!
//! JSON-RPC 2.0 server for the Model Context Protocol.

use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};

use lcsa_core::{
    ClipboardSignal, ContextApi, FocusSignal, SelectionSignal, SignalType, StructuralSignal,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::error::Error;
use crate::protocol::*;
use crate::{resources, tools};

/// Snapshot of the latest signals for quick access
#[derive(Debug, Clone, Default, Serialize)]
pub struct ContextSnapshot {
    pub latest_clipboard: Option<ClipboardSignal>,
    pub latest_selection: Option<SelectionSignal>,
    pub latest_focus: Option<FocusSignal>,
}

impl ContextSnapshot {
    pub fn as_summary(&self) -> Value {
        json!({
            "clipboard": self.latest_clipboard.as_ref().map(|s| json!({
                "content_type": s.content_type.as_str(),
                "size_bytes": s.size_bytes,
                "source_app": s.source_app,
                "likely_sensitive": s.likely_sensitive,
                "likely_command": s.likely_command
            })),
            "selection": self.latest_selection.as_ref().map(|s| json!({
                "content_type": s.content_type.as_str(),
                "size_bytes": s.size_bytes,
                "source_app": s.source_app,
                "likely_sensitive": s.likely_sensitive,
                "is_editable": s.is_editable
            })),
            "focus": self.latest_focus.as_ref().map(|s| json!({
                "source_app": s.source_app,
                "target": s.target.as_str(),
                "is_editable": s.is_editable
            }))
        })
    }
}

/// MCP Server
pub struct McpServer {
    api: ContextApi,
    snapshot: Arc<Mutex<ContextSnapshot>>,
}

impl McpServer {
    pub fn new() -> Result<Self, Error> {
        let mut api = ContextApi::new()?;
        let snapshot = Arc::new(Mutex::new(ContextSnapshot::default()));

        // Subscribe to signals to keep snapshot updated
        for signal_type in [
            SignalType::Clipboard,
            SignalType::Selection,
            SignalType::Focus,
        ] {
            if api.is_signal_supported(signal_type) {
                let snapshot_clone = Arc::clone(&snapshot);
                let result = api.subscribe_enveloped(signal_type, move |envelope| {
                    let mut snapshot = snapshot_clone.lock().unwrap();
                    match envelope.payload {
                        StructuralSignal::Clipboard(s) => snapshot.latest_clipboard = Some(s),
                        StructuralSignal::Selection(s) => snapshot.latest_selection = Some(s),
                        StructuralSignal::Focus(s) => snapshot.latest_focus = Some(s),
                        _ => {}
                    }
                });

                if let Err(e) = result {
                    tracing::warn!("Failed to subscribe to {:?}: {}", signal_type, e);
                }
            }
        }

        Ok(Self { api, snapshot })
    }

    /// Run the MCP server using stdio transport
    pub fn run_stdio(&mut self) -> Result<(), Error> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = BufReader::new(stdin.lock());

        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            tracing::debug!("Received: {}", line);

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let response = JsonRpcResponse::error(
                        None,
                        JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {}", e),
                            data: None,
                        },
                    );
                    self.send_response(&mut stdout, &response)?;
                    continue;
                }
            };

            let response = self.handle_request(request);
            self.send_response(&mut stdout, &response)?;
        }

        Ok(())
    }

    fn send_response(
        &self,
        stdout: &mut std::io::Stdout,
        response: &JsonRpcResponse,
    ) -> Result<(), Error> {
        let output = serde_json::to_string(response)?;
        tracing::debug!("Sending: {}", output);
        writeln!(stdout, "{}", output)?;
        stdout.flush()?;
        Ok(())
    }

    fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params),
            "initialized" => Ok(json!({})),
            "resources/list" => self.handle_list_resources(),
            "resources/read" => self.handle_read_resource(request.params),
            "tools/list" => self.handle_list_tools(),
            "tools/call" => self.handle_call_tool(request.params),
            method => Err(JsonRpcError::method_not_found(method)),
        };

        match result {
            Ok(value) => JsonRpcResponse::success(request.id, value),
            Err(error) => JsonRpcResponse::error(request.id, error),
        }
    }

    fn handle_initialize(&self, _params: Value) -> Result<Value, JsonRpcError> {
        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                resources: Some(ResourceCapabilities {
                    subscribe: false,
                    list_changed: false,
                }),
                tools: Some(ToolCapabilities {}),
            },
            server_info: ServerInfo {
                name: "lcsa-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        serde_json::to_value(result).map_err(|e| JsonRpcError::internal_error(e.to_string()))
    }

    fn handle_list_resources(&self) -> Result<Value, JsonRpcError> {
        Ok(json!({
            "resources": resources::list_resources()
        }))
    }

    fn handle_read_resource(&self, params: Value) -> Result<Value, JsonRpcError> {
        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("'uri' parameter required"))?;

        let snapshot = self.snapshot.lock().unwrap();
        let content = resources::read_resource(uri, &snapshot)
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        Ok(json!({
            "contents": [{
                "uri": uri,
                "mimeType": "application/json",
                "text": content
            }]
        }))
    }

    fn handle_list_tools(&self) -> Result<Value, JsonRpcError> {
        Ok(json!({
            "tools": tools::list_tools()
        }))
    }

    fn handle_call_tool(&mut self, params: Value) -> Result<Value, JsonRpcError> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("'name' parameter required"))?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let snapshot = self.snapshot.lock().unwrap().clone();
        let result = tools::call_tool(name, arguments, &mut self.api, &snapshot)
            .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result).unwrap_or_default()
            }]
        }))
    }
}
