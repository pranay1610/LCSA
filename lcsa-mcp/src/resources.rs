//! MCP Resources
//!
//! LCSA signal resources exposed via MCP.

use serde::Serialize;

use crate::error::Error;
use crate::server::ContextSnapshot;

/// MCP Resource definition
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// List all available resources
pub fn list_resources() -> Vec<Resource> {
    vec![
        Resource {
            uri: "lcsa://context/current".to_string(),
            name: "Current Context".to_string(),
            description: Some(
                "Snapshot of all latest signals (clipboard, selection, focus)".to_string(),
            ),
            mime_type: Some("application/json".to_string()),
        },
        Resource {
            uri: "lcsa://signals/latest/clipboard".to_string(),
            name: "Latest Clipboard Signal".to_string(),
            description: Some(
                "Most recent clipboard change signal (metadata only, not content)".to_string(),
            ),
            mime_type: Some("application/json".to_string()),
        },
        Resource {
            uri: "lcsa://signals/latest/selection".to_string(),
            name: "Latest Selection Signal".to_string(),
            description: Some("Most recent text selection signal".to_string()),
            mime_type: Some("application/json".to_string()),
        },
        Resource {
            uri: "lcsa://signals/latest/focus".to_string(),
            name: "Latest Focus Signal".to_string(),
            description: Some("Most recent window focus change signal".to_string()),
            mime_type: Some("application/json".to_string()),
        },
    ]
}

/// Read a resource by URI
pub fn read_resource(uri: &str, snapshot: &ContextSnapshot) -> Result<String, Error> {
    match uri {
        "lcsa://context/current" => {
            serde_json::to_string_pretty(&snapshot.as_summary()).map_err(Error::from)
        }
        "lcsa://signals/latest/clipboard" => snapshot
            .latest_clipboard
            .as_ref()
            .map(|s| serde_json::to_string_pretty(s))
            .transpose()
            .map_err(Error::from)?
            .ok_or_else(|| {
                Error::ResourceNotFound("No clipboard signal available yet".to_string())
            }),
        "lcsa://signals/latest/selection" => snapshot
            .latest_selection
            .as_ref()
            .map(|s| serde_json::to_string_pretty(s))
            .transpose()
            .map_err(Error::from)?
            .ok_or_else(|| {
                Error::ResourceNotFound("No selection signal available yet".to_string())
            }),
        "lcsa://signals/latest/focus" => snapshot
            .latest_focus
            .as_ref()
            .map(|s| serde_json::to_string_pretty(s))
            .transpose()
            .map_err(Error::from)?
            .ok_or_else(|| Error::ResourceNotFound("No focus signal available yet".to_string())),
        _ => Err(Error::ResourceNotFound(uri.to_string())),
    }
}
