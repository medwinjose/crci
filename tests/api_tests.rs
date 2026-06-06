use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use std::collections::{HashMap, VecDeque};
use std::sync::{atomic::AtomicU64, Arc, Mutex, RwLock};
use std::time::Instant;
use tokio::sync::broadcast;
use tower::ServiceExt;

use crci::api::{build_router, ApiMessage, ApiState};

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
async fn health_endpoint_returns_ok() {
    let app = build_router(setup_state());
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn divergences_empty_returns_array() {
    let app = build_router(setup_state());
    let req = Request::builder()
        .uri("/divergences")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn chain_head_returns_valid_hex() {
    let app = build_router(setup_state());
    let req = Request::builder()
        .uri("/chain/head")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let head_hash = json["head_hash"].as_str().unwrap();
    assert_eq!(head_hash.len(), 64);
}

#[tokio::test]
async fn unknown_route_returns_404_json() {
    let app = build_router(setup_state());
    let req = Request::builder()
        .uri("/nonexistent")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("error").is_some());
}

#[tokio::test]
async fn oversized_body_returns_413() {
    let app = build_router(setup_state());
    // Generate a 65KB string and quote it so it's valid JSON
    let big_body_str = format!("\"{}\"", "x".repeat(65 * 1024));
    let len = big_body_str.len();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/simulate/panic")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_LENGTH, len.to_string())
        .body(Body::from(big_body_str))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("error").is_some());
}

#[tokio::test]
async fn rate_limit_triggers_429() {
    let app = build_router(setup_state());
    // For router tests where state/middleware is evaluated repeatedly, we need
    // an app that supports calling repeatedly. `oneshot` consumes the service.
    // However, axum's Router implements Clone, but calling oneshot multiple times
    // on app.clone() is fine.
    for _ in 0..60 {
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    // 61st request should be 429
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(res.headers().contains_key(header::RETRY_AFTER));
}

#[tokio::test]
async fn response_has_request_id_header() {
    let app = build_router(setup_state());
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let req_id = res.headers().get("X-Request-Id");
    assert!(req_id.is_some());
    assert!(!req_id.unwrap().to_str().unwrap().is_empty());
}
