import sys

content = """
#[tokio::test]
async fn test_status_returns_200() {
    let app = build_router(setup_state());
    let req = Request::builder()
        .uri("/api/v1/status")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let _: crci::api_types::NodeStatusResponse = serde_json::from_slice(&body).unwrap();
}

#[tokio::test]
async fn test_peers_returns_200() {
    let app = build_router(setup_state());
    let req = Request::builder()
        .uri("/api/v1/peers")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let _: crci::api_types::PeerListResponse = serde_json::from_slice(&body).unwrap();
}

#[tokio::test]
async fn test_inject_valid() {
    let app = build_router(setup_state());
    let req_body = crci::api_types::InjectRequest {
        payload: "hello world".to_string(),
        priority: 1,
    };
    let json_body = serde_json::to_string(&req_body).unwrap();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/inject")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json_body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn test_inject_payload_too_large() {
    let app = build_router(setup_state());
    let big_payload = "x".repeat(9000);
    let req_body = crci::api_types::InjectRequest {
        payload: big_payload,
        priority: 1,
    };
    let json_body = serde_json::to_string(&req_body).unwrap();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/inject")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json_body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert!(res.status() == StatusCode::BAD_REQUEST || res.status() == StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn test_inject_invalid_priority() {
    let app = build_router(setup_state());
    let req_body = crci::api_types::InjectRequest {
        payload: "hi".to_string(),
        priority: 9,
    };
    let json_body = serde_json::to_string(&req_body).unwrap();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/inject")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json_body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_inject_wrong_content_type() {
    let app = build_router(setup_state());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/inject")
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from("hello"))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}
"""

with open("tests/api_tests.rs", "a") as f:
    f.write(content)
