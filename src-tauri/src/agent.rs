use crate::{model, service::Runtime, storage};
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
struct AgentState {
    runtime: Arc<Runtime>,
    token: String,
}
async fn authorize(State(state): State<AgentState>, request: Request, next: Next) -> Response {
    let expected = format!("Bearer {}", state.token);
    let supplied = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let equal = supplied.len() == expected.len()
        && supplied
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0;
    if !equal || request.headers().contains_key(header::ORIGIN) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"Local agent authentication required."})),
        )
            .into_response();
    }
    next.run(request).await
}
type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;
fn response(result: Result<Value, String>) -> ApiResult {
    result
        .map(Json)
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(json!({"error":error}))))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Send {
    repo: String,
    paths: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChatInput {
    name: String,
    members: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportInput {
    repo: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receive {
    key: String,
    directory: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Retry {
    key: String,
    recipient: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    job_id: String,
}
async fn state(State(s): State<AgentState>) -> Json<Value> {
    Json(serde_json::to_value(s.runtime.snapshot()).unwrap_or(Value::Null))
}
async fn send(State(s): State<AgentState>, Json(b): Json<Send>) -> ApiResult {
    response(
        s.runtime
            .send(b.repo, b.paths)
            .await
            .map(|key| json!({"key":key})),
    )
}
async fn receive(State(s): State<AgentState>, Json(b): Json<Receive>) -> ApiResult {
    response(
        s.runtime
            .receive(b.key, b.directory)
            .await
            .map(|directory| json!({"directory":directory})),
    )
}
async fn retry(State(s): State<AgentState>, Json(b): Json<Retry>) -> ApiResult {
    response(
        s.runtime
            .retry(b.key, b.recipient)
            .await
            .map(|_| json!({"ok":true})),
    )
}
async fn request_retry(State(s): State<AgentState>, Json(b): Json<Key>) -> ApiResult {
    response(
        s.runtime
            .request_retry(b.key)
            .await
            .map(|_| json!({"ok":true})),
    )
}
async fn cancel(State(s): State<AgentState>, Json(b): Json<Job>) -> ApiResult {
    response(s.runtime.cancel(b.job_id).map(|_| json!({"ok":true})))
}
async fn sync(State(s): State<AgentState>) -> ApiResult {
    response(s.runtime.sync(true).await.map(|_| json!({"ok":true})))
}
async fn create_chat(State(s): State<AgentState>, Json(b): Json<ChatInput>) -> ApiResult {
    response(
        s.runtime
            .create_chat(b.name, b.members)
            .await
            .and_then(|chat| serde_json::to_value(chat).map_err(|e| e.to_string())),
    )
}
async fn import_chat(State(s): State<AgentState>, Json(b): Json<ImportInput>) -> ApiResult {
    response(
        s.runtime
            .import_chat(b.repo)
            .await
            .and_then(|chat| serde_json::to_value(chat).map_err(|e| e.to_string())),
    )
}
pub async fn serve(runtime: Arc<Runtime>) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let token = format!("{}{}", model::id(), model::id());
    storage::write_private(&runtime.dir.join("agent.json"), &serde_json::to_vec(&json!({"url":format!("http://127.0.0.1:{port}"),"token":token,"pid":std::process::id(),"version":1})).map_err(|e| e.to_string())?)?;
    let state = AgentState { runtime, token };
    let router = Router::new()
        .route("/v1/state", get(self::state))
        .route("/v1/chats", post(create_chat))
        .route("/v1/chats/import", post(import_chat))
        .route("/v1/send", post(send))
        .route("/v1/receive", post(receive))
        .route("/v1/retry", post(retry))
        .route("/v1/request-retry", post(request_retry))
        .route("/v1/cancel", post(cancel))
        .route("/v1/sync", post(sync))
        .layer(DefaultBodyLimit::max(128 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), authorize))
        .with_state(state);
    axum::serve(listener, router)
        .await
        .map_err(|e| e.to_string())
}
