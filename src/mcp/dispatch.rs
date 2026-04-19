use serde_json::{json, Value};

use super::context::ContextWindow;
use super::{tools, transport};
use crate::store::concept::ConceptStore;
use crate::store::layer::LayerStore;
use crate::store::ContentStore;
use crate::trie::Trie;

const SERVER_NAME: &str = "trie-memory";
const SERVER_VERSION: &str = "0.1.0";

/// Handle a JSON-RPC message and return an optional response.
/// Shared by both stdio and SSE transports.
pub fn handle_message(
    trie: &mut Trie,
    word_trie: &mut Trie,
    store: &mut ContentStore,
    concepts: &mut ConceptStore,
    layers: &mut LayerStore,
    context: &mut ContextWindow,
    msg: &Value,
) -> Option<Value> {
    let method = msg.get("method")?.as_str()?;
    let id = msg.get("id");

    match method {
        "initialize" => {
            let id = id?;
            Some(transport::success_response(
                id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "version": SERVER_VERSION,
                    }
                }),
            ))
        }

        "notifications/initialized" => None,

        "tools/list" => {
            let id = id?;
            Some(transport::success_response(id, tools::tool_list()))
        }

        "tools/call" => {
            let id = id?;
            let params = msg.get("params")?;
            let name = params.get("name")?.as_str()?;
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            let result = tools::call_tool(
                trie, word_trie, store, concepts, layers, context, name, &arguments,
            );
            Some(transport::success_response(id, result))
        }

        _ => {
            if let Some(id) = id {
                Some(transport::error_response(
                    id,
                    -32601,
                    &format!("Method not found: {}", method),
                ))
            } else {
                None
            }
        }
    }
}
