use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket},
        ConnectInfo, DefaultBodyLimit, Path, Request, State, WebSocketUpgrade,
    },
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::{atomic::AtomicU64, Arc, Mutex, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::api_types::*;
use crate::merkle::DivergenceAlert;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut res = (status, Json(self)).into_response();
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        res
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiMessage {
    pub from: String, // pubkey hex
    pub severity: u8,
    pub content: String,
    pub timestamp_ms: u64,
}

pub struct ApiState {
    pub node_count: Arc<RwLock<usize>>,
    pub peer_list: Arc<RwLock<Vec<String>>>,
    pub recent_messages: Arc<RwLock<VecDeque<ApiMessage>>>,
    pub ws_tx: broadcast::Sender<String>,
    // Session 42 fields
    pub node_id: String,
    pub start_time: Instant,
    pub byzantine_events: Arc<AtomicU64>,
    pub divergence_alerts: Arc<RwLock<Vec<DivergenceAlert>>>,
    pub divergence_tx: broadcast::Sender<DivergenceAlert>,
    pub rate_limit_counts: Arc<Mutex<HashMap<IpAddr, (usize, u64)>>>,
    pub merkle_head: Arc<RwLock<([u8; 32], u64)>>,
}

#[derive(Serialize, Deserialize)]
pub struct StatusResponse {
    pub node_id: String,
    pub uptime_secs: u64,
    pub node_count: usize,
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct PeersResponse {
    pub peers: Vec<String>,
    pub count: usize,
}

#[derive(Serialize, Deserialize)]
pub struct MessagesResponse {
    pub messages: Vec<ApiMessage>,
    pub count: usize,
}

#[derive(Serialize, Deserialize)]
pub struct PanicResponse {
    pub queued: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

pub struct LegacyApiError {
    pub status: StatusCode,
    pub message: String,
    pub code: String,
    pub retry_after: Option<u64>,
}

impl IntoResponse for LegacyApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: self.message,
            code: self.code,
        });
        let mut response = (self.status, body).into_response();
        if let Some(retry) = self.retry_after {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                HeaderValue::from_str(&retry.to_string()).unwrap(),
            );
        }
        response
    }
}

#[derive(Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub node_id: String,
    pub uptime_secs: u64,
    pub byzantine_events: u64,
    pub divergence_alerts: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ChainHeadResponse {
    pub head_hash: String,
    pub head_seq: u64,
}

async fn hardening_middleware(req: Request, next: Next) -> Response {
    let is_post = req.method() == Method::POST || req.method() == Method::PUT;
    let node_id = if let Some(state) = req.extensions().get::<Arc<ApiState>>() {
        state.node_id.clone()
    } else {
        "unknown".to_string()
    };

    if is_post {
        if let Some(ct) = req.headers().get(header::CONTENT_TYPE) {
            if ct != "application/json" {
                let err = ApiError {
                    code: 415,
                    message: "Content-Type must be application/json".to_string(),
                };
                return err.into_response();
            }
        } else {
            let err = ApiError {
                code: 415,
                message: "Content-Type must be application/json".to_string(),
            };
            return err.into_response();
        }
    }

    let mut res = next.run(req).await;

    let req_id = Uuid::new_v4().to_string();
    res.headers_mut()
        .insert("X-Request-Id", HeaderValue::from_str(&req_id).unwrap());
    res.headers_mut()
        .insert("X-CRCI-Version", HeaderValue::from_static("0.1.0"));
    if is_post {
        res.headers_mut()
            .insert("X-CRCI-Node-Id", HeaderValue::from_str(&node_id).unwrap());
    }

    if res.status() == StatusCode::PAYLOAD_TOO_LARGE {
        let error = LegacyApiError {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            message: "Payload too large".to_string(),
            code: "PAYLOAD_TOO_LARGE".to_string(),
            retry_after: None,
        };
        let mut new_res = error.into_response();
        if is_post {
            new_res
                .headers_mut()
                .insert("X-CRCI-Node-Id", HeaderValue::from_str(&node_id).unwrap());
        }
        new_res
            .headers_mut()
            .insert("X-Request-Id", HeaderValue::from_str(&req_id).unwrap());
        new_res
            .headers_mut()
            .insert("X-CRCI-Version", HeaderValue::from_static("0.1.0"));
        return new_res;
    }

    if res.status() == StatusCode::NOT_FOUND && res.headers().get(header::CONTENT_TYPE).is_none() {
        let error = LegacyApiError {
            status: StatusCode::NOT_FOUND,
            message: "Not Found".to_string(),
            code: "NOT_FOUND".to_string(),
            retry_after: None,
        };
        let mut new_res = error.into_response();
        new_res
            .headers_mut()
            .insert("X-Request-Id", HeaderValue::from_str(&req_id).unwrap());
        new_res
            .headers_mut()
            .insert("X-CRCI-Version", HeaderValue::from_static("0.1.0"));
        if is_post {
            new_res
                .headers_mut()
                .insert("X-CRCI-Node-Id", HeaderValue::from_str(&node_id).unwrap());
        }
        return new_res;
    }

    res
}

async fn rate_limit_middleware(req: Request, next: Next) -> Response {
    let state = req.extensions().get::<Arc<ApiState>>().unwrap();
    let ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .unwrap_or_else(|| std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let rate_limit_error = {
        let mut counts = state.rate_limit_counts.lock().unwrap();
        let entry = counts.entry(ip).or_insert((0, now));

        if now >= entry.1 + 60 {
            entry.0 = 0;
            entry.1 = now;
        }

        if entry.0 >= 60 {
            let retry = 60 - (now - entry.1);
            Some(retry)
        } else {
            entry.0 += 1;
            None
        }
    }; // MutexGuard dropped here

    if let Some(retry) = rate_limit_error {
        return LegacyApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Rate limit exceeded".to_string(),
            code: "RATE_LIMIT_EXCEEDED".to_string(),
            retry_after: Some(retry),
        }
        .into_response();
    }

    next.run(req).await
}

pub fn build_router(state: Arc<ApiState>) -> Router {
    let app = Router::new()
        .route("/status", get(status_handler))
        .route("/peers", get(peers_handler))
        .route("/messages", get(messages_handler))
        .route("/simulate/panic", post(simulate_panic_handler))
        .route("/ws", get(ws_handler))
        .route("/health", get(health_handler))
        .route("/divergences", get(divergences_handler))
        .route("/divergences/:peer_id", get(divergences_peer_handler))
        .route("/chain/head", get(chain_head_handler))
        .route("/ws/divergences", get(ws_divergences_handler))
        .route("/api/v1/status", get(status_v1_handler))
        .route("/api/v1/peers", get(peers_v1_handler))
        .route(
            "/api/v1/inject",
            post(inject_v1_handler).layer(DefaultBodyLimit::max(8192)),
        )
        .route("/api/v1/openapi.json", get(openapi_handler))
        .with_state(state.clone());

    app.layer(middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(state))
        .layer(middleware::from_fn(hardening_middleware))
        .layer(DefaultBodyLimit::max(64 * 1024))
}

async fn status_handler(State(state): State<Arc<ApiState>>) -> Json<StatusResponse> {
    let node_count = *state.node_count.read().unwrap();
    Json(StatusResponse {
        node_id: state.node_id.clone(),
        uptime_secs: state.start_time.elapsed().as_secs(),
        node_count,
        version: "0.1.0".to_string(),
    })
}

async fn peers_handler(State(state): State<Arc<ApiState>>) -> Json<PeersResponse> {
    let peers = state.peer_list.read().unwrap().clone();
    let count = peers.len();
    Json(PeersResponse { peers, count })
}

async fn messages_handler(State(state): State<Arc<ApiState>>) -> Json<MessagesResponse> {
    let messages: Vec<_> = state
        .recent_messages
        .read()
        .unwrap()
        .iter()
        .cloned()
        .collect();
    let count = messages.len();
    Json(MessagesResponse { messages, count })
}

async fn simulate_panic_handler(
    State(state): State<Arc<ApiState>>,
    _body: String,
) -> Json<PanicResponse> {
    let msg = ApiMessage {
        from: state.node_id.clone(),
        severity: 5,
        content: "simulated panic".to_string(),
        timestamp_ms: 0,
    };
    if let Ok(json) = serde_json::to_string(&msg) {
        let _ = state.ws_tx.send(json);
    }
    Json(PanicResponse { queued: true })
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<ApiState>) {
    let initial_msgs: Vec<_> = {
        let msgs = state.recent_messages.read().unwrap();
        msgs.iter().rev().take(10).rev().cloned().collect()
    };

    for msg in initial_msgs {
        if let Ok(json) = serde_json::to_string(&msg) {
            if socket.send(WsMessage::Text(json)).await.is_err() {
                return;
            }
        }
    }

    let mut rx = state.ws_tx.subscribe();
    while let Ok(msg_text) = rx.recv().await {
        if socket.send(WsMessage::Text(msg_text)).await.is_err() {
            break;
        }
    }
}

async fn health_handler(State(state): State<Arc<ApiState>>) -> Json<HealthResponse> {
    let uptime_secs = state.start_time.elapsed().as_secs();
    let byzantine_events = state
        .byzantine_events
        .load(std::sync::atomic::Ordering::Relaxed);
    let divergence_alerts = state.divergence_alerts.read().unwrap().len() as u64;

    Json(HealthResponse {
        status: "ok".to_string(),
        node_id: state.node_id.clone(),
        uptime_secs,
        byzantine_events,
        divergence_alerts,
    })
}

async fn divergences_handler(State(state): State<Arc<ApiState>>) -> Json<Vec<DivergenceAlert>> {
    let mut alerts = state.divergence_alerts.read().unwrap().clone();
    alerts.reverse(); // newest first
    Json(alerts)
}

async fn divergences_peer_handler(
    State(state): State<Arc<ApiState>>,
    Path(peer_id): Path<String>,
) -> Result<Json<Vec<DivergenceAlert>>, LegacyApiError> {
    let alerts: Vec<_> = state
        .divergence_alerts
        .read()
        .unwrap()
        .iter()
        .filter(|a| a.peer_id == peer_id)
        .cloned()
        .collect();

    if alerts.is_empty() {
        return Err(LegacyApiError {
            status: StatusCode::NOT_FOUND,
            message: format!("No alerts found for peer: {}", peer_id),
            code: "PEER_NOT_FOUND".to_string(),
            retry_after: None,
        });
    }

    Ok(Json(alerts))
}

async fn chain_head_handler(State(state): State<Arc<ApiState>>) -> Json<ChainHeadResponse> {
    let head = *state.merkle_head.read().unwrap();
    let head_hash = head.0.iter().map(|b| format!("{:02x}", b)).collect();
    Json(ChainHeadResponse {
        head_hash,
        head_seq: head.1,
    })
}

async fn ws_divergences_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_divergences_socket(socket, state))
}

async fn handle_divergences_socket(mut socket: WebSocket, state: Arc<ApiState>) {
    let current_alerts: Vec<_> = state.divergence_alerts.read().unwrap().clone();
    if let Ok(json) = serde_json::to_string(&current_alerts) {
        if socket.send(WsMessage::Text(json)).await.is_err() {
            return;
        }
    }

    let mut rx = state.divergence_tx.subscribe();
    while let Ok(alert) = rx.recv().await {
        if let Ok(json) = serde_json::to_string(&alert) {
            if socket.send(WsMessage::Text(json)).await.is_err() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn setup_state() -> Arc<ApiState> {
        let (ws_tx, _) = broadcast::channel(100);
        let (divergence_tx, _) = broadcast::channel(100);
        let mut msgs = VecDeque::new();
        msgs.push_back(ApiMessage {
            from: "test".to_string(),
            severity: 1,
            content: "hello".to_string(),
            timestamp_ms: 123,
        });
        Arc::new(ApiState {
            node_count: Arc::new(RwLock::new(5)),
            peer_list: Arc::new(RwLock::new(vec!["peer1".to_string(), "peer2".to_string()])),
            recent_messages: Arc::new(RwLock::new(msgs)),
            ws_tx,
            node_id: "test-node".to_string(),
            start_time: Instant::now(),
            byzantine_events: Arc::new(AtomicU64::new(0)),
            divergence_alerts: Arc::new(RwLock::new(Vec::new())),
            divergence_tx,
            rate_limit_counts: Arc::new(Mutex::new(HashMap::new())),
            merkle_head: Arc::new(RwLock::new(([0; 32], 0))),
        })
    }

    #[tokio::test]
    async fn test_status_endpoint() {
        let app = build_router(setup_state());
        let request = Request::builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let status: StatusResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(status.version, "0.1.0");
        assert_eq!(status.node_count, 5);
    }
}

async fn status_v1_handler(State(state): State<Arc<ApiState>>) -> Json<NodeStatusResponse> {
    let peer_count = state.peer_list.read().unwrap().len();
    let uptime_secs = state.start_time.elapsed().as_secs();
    let message_count = state.recent_messages.read().unwrap().len() as u64;
    let head = *state.merkle_head.read().unwrap();
    let chain_head = head.0.iter().map(|b| format!("{:02x}", b)).collect();

    Json(NodeStatusResponse {
        node_id: state.node_id.clone(),
        peer_count,
        uptime_secs,
        message_count,
        chain_head,
    })
}

async fn peers_v1_handler(State(state): State<Arc<ApiState>>) -> Json<PeerListResponse> {
    let peers: Vec<PeerInfo> = state
        .peer_list
        .read()
        .unwrap()
        .iter()
        .map(|p| PeerInfo {
            peer_id: p.clone(),
            addr: "unknown".to_string(),
            last_seen_secs: state.start_time.elapsed().as_secs(),
            reputation: 1.0,
        })
        .collect();

    Json(PeerListResponse { peers })
}

async fn inject_v1_handler(
    State(_state): State<Arc<ApiState>>,
    Json(payload): Json<InjectRequest>,
) -> Result<(StatusCode, Json<InjectResponse>), ApiError> {
    if payload.payload.len() > 4096 {
        return Err(ApiError {
            code: 400,
            message: "Payload exceeds 4096 bytes".to_string(),
        });
    }
    if payload.priority > 3 {
        return Err(ApiError {
            code: 400,
            message: "Priority must be 0-3".to_string(),
        });
    }

    let message_id = uuid::Uuid::new_v4().to_string();
    let queued_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    Ok((
        StatusCode::ACCEPTED,
        Json(InjectResponse {
            message_id,
            queued_at,
        }),
    ))
}

async fn openapi_handler(
) -> Result<(header::HeaderMap, String), (StatusCode, Json<serde_json::Value>)> {
    match std::fs::read_to_string("docs/openapi.json") {
        Ok(content) => {
            let mut headers = header::HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            Ok((headers, content))
        }
        Err(_) => {
            let err_body = serde_json::json!({"error": "spec unavailable"});
            Err((StatusCode::SERVICE_UNAVAILABLE, Json(err_body)))
        }
    }
}
