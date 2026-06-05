use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

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
}

pub fn build_router(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/status", get(status_handler))
        .route("/peers", get(peers_handler))
        .route("/messages", get(messages_handler))
        .route("/simulate/panic", post(simulate_panic_handler))
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn status_handler(State(state): State<Arc<ApiState>>) -> Json<StatusResponse> {
    let node_count = *state.node_count.read().unwrap();
    Json(StatusResponse {
        node_id: "local-node".to_string(),
        uptime_secs: 0,
        node_count,
        version: "0.37.0".to_string(),
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

async fn simulate_panic_handler(State(state): State<Arc<ApiState>>) -> Json<PanicResponse> {
    let msg = ApiMessage {
        from: "local-node".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt; // for `oneshot` and `ready` // for `collect`

    fn setup_state() -> Arc<ApiState> {
        let (ws_tx, _) = broadcast::channel(100);
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

        assert_eq!(status.version, "0.37.0");
        assert_eq!(status.node_count, 5);
    }

    #[tokio::test]
    async fn test_peers_endpoint() {
        let app = build_router(setup_state());
        let request = Request::builder()
            .uri("/peers")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let peers_res: PeersResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(peers_res.count, 2);
        assert_eq!(peers_res.peers.len(), 2);
    }

    #[tokio::test]
    async fn test_messages_endpoint() {
        let app = build_router(setup_state());
        let request = Request::builder()
            .uri("/messages")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let msgs_res: MessagesResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(msgs_res.count, 1);
    }

    #[tokio::test]
    async fn test_ws_connect() {
        // We MUST spin up a listener for WebSocket testing because axum's WebSocketUpgrade
        // requires hyper's OnUpgrade extension which is not present in oneshot() mock requests.
        let app = build_router(setup_state());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let req = format!(
            "GET /ws HTTP/1.1\r\n\
             Host: localhost\r\n\
             Connection: upgrade\r\n\
             Upgrade: websocket\r\n\
             Sec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             \r\n"
        );
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream.write_all(req.as_bytes()).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let response = String::from_utf8_lossy(&buf[..n]);
        assert!(response.contains("101 Switching Protocols"));

        // Read the first WebSocket frame (the replay payload)
        let n2 = stream.read(&mut buf).await.unwrap();
        assert!(n2 > 0);
        assert_eq!(buf[0], 0x81); // 0x81 is FIN + Text frame
    }
}
