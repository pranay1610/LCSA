//! MCP Tools
//!
//! LCSA actions exposed as MCP tools.

use lcsa_core::{Capability, ContextApi, PermissionRequest, Scope};
use serde::Serialize;
use serde_json::{Value, json};

use crate::error::Error;
use crate::server::ContextSnapshot;

/// MCP Tool definition
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

/// List all available tools
pub fn list_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_supported_signals".to_string(),
            description: Some("Query which signal types are supported on this platform".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_current_context".to_string(),
            description: Some("Get a snapshot of all latest signals (clipboard, selection, focus)".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_clipboard_content".to_string(),
            description: Some("Get raw clipboard content (requires permission). Use sparingly and only when needed.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "reason": {
                        "type": "string",
                        "description": "Explanation for why clipboard content access is needed"
                    }
                },
                "required": ["reason"]
            }),
        },
    ]
}

/// Call a tool by name
pub fn call_tool(
    name: &str,
    arguments: Value,
    api: &mut ContextApi,
    snapshot: &ContextSnapshot,
) -> Result<Value, Error> {
    match name {
        "get_supported_signals" => {
            let supported = api.supported_signals();
            let details: Vec<Value> = supported
                .iter()
                .map(|s| {
                    json!({
                        "type": s.as_str(),
                        "support": api.signal_support(*s).to_string()
                    })
                })
                .collect();

            Ok(json!({
                "signals": supported.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                "details": details
            }))
        }

        "get_current_context" => Ok(json!({
            "device": {
                "id": api.device_context().device_id,
                "name": api.device_context().device_name,
                "platform": api.device_context().platform.as_str()
            },
            "application": {
                "id": api.application_context().app_id,
                "name": api.application_context().app_name
            },
            "supported_signals": api.supported_signals().iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            "latest_signals": snapshot.as_summary()
        })),

        "get_clipboard_content" => {
            let reason = arguments
                .get("reason")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::InvalidArgument("'reason' parameter is required".to_string())
                })?;

            // Request permission if not already granted
            if !api.can_access(Capability::ReadClipboardContent) {
                api.request_permission(PermissionRequest::new(
                    Capability::ReadClipboardContent,
                    Scope::Session,
                    reason,
                ))?;
            }

            let content = api.read_clipboard_content()?;
            Ok(json!({
                "preview": content.redacted_preview(),
                "source_app": content.source_app,
                "payload": content.payload
            }))
        }

        _ => Err(Error::UnknownTool(name.to_string())),
    }
}
