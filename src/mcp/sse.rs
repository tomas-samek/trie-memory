use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use serde_json::Value;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use super::context::ContextWindow;
use super::dispatch;
use crate::store::concept::ConceptStore;
use crate::store::layer::LayerStore;
use crate::store::ContentStore;
use crate::trie::Trie;

type Sessions = Arc<Mutex<HashMap<String, mpsc::Sender<Value>>>>;

#[derive(Clone)]
struct AppState {
    trie: Arc<Mutex<Trie>>,
    word_trie: Arc<Mutex<Trie>>,
    store: Arc<Mutex<ContentStore>>,
    concepts: Arc<Mutex<ConceptStore>>,
    layers: Arc<Mutex<LayerStore>>,
    context: Arc<Mutex<ContextWindow>>,
    sessions: Sessions,
}

/// Start the SSE MCP server on the given port.
pub async fn run(
    trie: Arc<Mutex<Trie>>,
    word_trie: Arc<Mutex<Trie>>,
    store: Arc<Mutex<ContentStore>>,
    concepts: Arc<Mutex<ConceptStore>>,
    layers: Arc<Mutex<LayerStore>>,
    context: Arc<Mutex<ContextWindow>>,
    port: u16,
) {
    let state = AppState {
        trie,
        word_trie,
        store,
        concepts,
        layers,
        context,
        sessions: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/sse", get(sse_handler))
        .route("/messages", post(messages_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    eprintln!(
        "trie-memory MCP server ready (SSE) on http://localhost:{}/sse",
        port
    );

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");
    axum::serve(listener, app).await.expect("Server error");
}

async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let session_id = generate_session_id();
    let (tx, rx) = mpsc::channel::<Value>(64);

    state
        .sessions
        .lock()
        .await
        .insert(session_id.clone(), tx);

    let endpoint_url = format!("/messages?sessionId={}", session_id);

    let endpoint_event = Event::default().event("endpoint").data(endpoint_url);

    let initial = tokio_stream::once(Ok(endpoint_event));

    let message_stream = ReceiverStream::new(rx).map(|val| {
        let data = serde_json::to_string(&val).unwrap_or_default();
        Ok(Event::default().event("message").data(data))
    });

    Sse::new(initial.chain(message_stream))
}

#[derive(serde::Deserialize)]
struct MessageQuery {
    #[serde(rename = "sessionId")]
    session_id: String,
}

async fn messages_handler(
    State(state): State<AppState>,
    Query(query): Query<MessageQuery>,
    body: String,
) -> impl IntoResponse {
    let tx = {
        let sessions = state.sessions.lock().await;
        match sessions.get(&query.session_id) {
            Some(tx) => tx.clone(),
            None => {
                return (StatusCode::NOT_FOUND, "Session not found".to_string());
            }
        }
    };

    let msg: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e));
        }
    };

    let response = {
        let mut trie = state.trie.lock().await;
        let mut word_trie = state.word_trie.lock().await;
        let mut store = state.store.lock().await;
        let mut concepts = state.concepts.lock().await;
        let mut layers = state.layers.lock().await;
        let mut context = state.context.lock().await;
        dispatch::handle_message(
            &mut trie,
            &mut word_trie,
            &mut store,
            &mut concepts,
            &mut layers,
            &mut context,
            &msg,
        )
    };

    if let Some(response) = response {
        if tx.send(response).await.is_err() {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "SSE channel closed".to_string(),
            );
        }
    }

    (StatusCode::ACCEPTED, "Accepted".to_string())
}

fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", nanos)
}
