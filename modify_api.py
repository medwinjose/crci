import re

with open("src/api.rs", "r") as f:
    content = f.read()

# 1. Rename ApiError to LegacyApiError
content = content.replace("pub struct ApiError {", "pub struct LegacyApiError {")
content = content.replace("impl IntoResponse for ApiError {", "impl IntoResponse for LegacyApiError {")
content = content.replace("ApiError {", "LegacyApiError {")
content = content.replace("Result<Json<Vec<DivergenceAlert>>, ApiError>", "Result<Json<Vec<DivergenceAlert>>, LegacyApiError>")

# 2. Add imports
content = content.replace("use crate::merkle::DivergenceAlert;", "use crate::merkle::DivergenceAlert;\nuse crate::api_types::*;")

# 3. Add into_response for crate::api_types::ApiError
into_res = """
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut res = (status, Json(self)).into_response();
        res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
        res
    }
}
"""
content = content.replace("use crate::api_types::*;", "use crate::api_types::*;\n" + into_res)

# 4. Modify hardening_middleware to add X-CRCI-Node-Id, limit body to 8192 for POST.
# Wait, DefaultBodyLimit might be better for 8192.
# Let's see the current hardening_middleware.
old_hardening = """async fn hardening_middleware(req: Request, next: Next) -> Response {
    if req.method() == Method::POST || req.method() == Method::PUT {
        if let Some(ct) = req.headers().get(header::CONTENT_TYPE) {
            if ct != "application/json" {
                return LegacyApiError {"""

new_hardening = """async fn hardening_middleware(req: Request, next: Next) -> Response {
    let is_post = req.method() == Method::POST;
    let node_id = if let Some(state) = req.extensions().get::<Arc<ApiState>>() {
        state.node_id.clone()
    } else {
        "unknown".to_string()
    };

    if req.method() == Method::POST || req.method() == Method::PUT {
        if let Some(ct) = req.headers().get(header::CONTENT_TYPE) {
            if ct != "application/json" {
                let err = ApiError { code: 415, message: "Content-Type must be application/json".to_string() };
                return err.into_response();
            }
        } else {
            let err = ApiError { code: 415, message: "Content-Type must be application/json".to_string() };
            return err.into_response();
        }
    }

    let mut res = next.run(req).await;"""
content = content.replace(old_hardening, new_hardening)

# 5. Add X-CRCI-Node-Id to POST responses
# Find res.headers_mut().insert("X-CRCI-Version", HeaderValue::from_static("0.1.0"));
# There are 3 occurrences. Let's just do a regex replace.
content = re.sub(
    r'(res\.headers_mut\(\)\s*\.insert\("X-CRCI-Version", HeaderValue::from_static\("0\.1\.0"\)\);)',
    r'\1\n    if is_post { res.headers_mut().insert("X-CRCI-Node-Id", HeaderValue::from_str(&node_id).unwrap()); }',
    content,
    count=1
)

# For the other ones (PAYLOAD_TOO_LARGE and NOT_FOUND), they are `new_res`
content = re.sub(
    r'(new_res\s*\.headers_mut\(\)\s*\.insert\("X-CRCI-Version", HeaderValue::from_static\("0\.1\.0"\)\);)',
    r'\1\n        if is_post { new_res.headers_mut().insert("X-CRCI-Node-Id", HeaderValue::from_str(&node_id).unwrap()); }',
    content
)

# Also need to handle payload size > 8192. We'll do it by adding DefaultBodyLimit::max(8192) to the inject route, or change global.
# Global is 64 * 1024.
# Wait, "Reject body > 8192 bytes → 413" for POST. Let's just check the Content-Length in the middleware if it exists.
# But Content-Length can be faked. Let's rely on axum's DefaultBodyLimit. But we don't want to break other routes.
# Let's add axum::extract::DefaultBodyLimit::max(8192) to the post route only!

# 6. Add new endpoints to build_router
router_old = """.route("/ws/divergences", get(ws_divergences_handler))
        .with_state(state.clone());"""
router_new = """.route("/ws/divergences", get(ws_divergences_handler))
        .route("/api/v1/status", get(status_v1_handler))
        .route("/api/v1/peers", get(peers_v1_handler))
        .route("/api/v1/inject", post(inject_v1_handler).layer(DefaultBodyLimit::max(8192)))
        .route("/api/v1/openapi.json", get(openapi_handler))
        .with_state(state.clone());"""
content = content.replace(router_old, router_new)

# 7. Add new handlers
handlers = """
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
    let peers: Vec<PeerInfo> = state.peer_list.read().unwrap().iter().map(|p| PeerInfo {
        peer_id: p.clone(),
        addr: "unknown".to_string(),
        last_seen_secs: state.start_time.elapsed().as_secs(),
        reputation: 1.0,
    }).collect();
    
    Json(PeerListResponse { peers })
}

async fn inject_v1_handler(
    State(_state): State<Arc<ApiState>>,
    Json(payload): Json<InjectRequest>
) -> Result<(StatusCode, Json<InjectResponse>), ApiError> {
    if payload.payload.len() > 4096 {
        return Err(ApiError {
            code: 400,
            message: "Payload exceeds 4096 bytes".to_string()
        });
    }
    if payload.priority > 3 {
        return Err(ApiError {
            code: 400,
            message: "Priority must be 0-3".to_string()
        });
    }
    
    let message_id = uuid::Uuid::new_v4().to_string();
    let queued_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    
    Ok((StatusCode::ACCEPTED, Json(InjectResponse { message_id, queued_at })))
}

async fn openapi_handler() -> Result<(header::HeaderMap, String), (StatusCode, Json<serde_json::Value>)> {
    match std::fs::read_to_string("docs/openapi.json") {
        Ok(content) => {
            let mut headers = header::HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
            Ok((headers, content))
        }
        Err(_) => {
            let err_body = serde_json::json!({"error": "spec unavailable"});
            Err((StatusCode::SERVICE_UNAVAILABLE, Json(err_body)))
        }
    }
}
"""
content = content + handlers

# In hardening_middleware, we changed ApiError for 415, let's also change PAYLOAD_TOO_LARGE and NOT_FOUND.
# Wait, LegacyApiError is still used for PAYLOAD_TOO_LARGE. But we want ApiError for POST? 
# The prompt says: "docs/openapi.yaml: 400/413/415 error responses for /api/v1/inject". 
# So ApiError { code: 413, message: "Payload too large" } is better. 
# Let's replace the LegacyApiError in hardening_middleware.
content = content.replace('LegacyApiError {\n            status: StatusCode::PAYLOAD_TOO_LARGE,\n            message: "Payload too large".to_string(),\n            code: "PAYLOAD_TOO_LARGE".to_string(),\n            retry_after: None,\n        };', 'ApiError {\n            code: 413,\n            message: "Payload too large".to_string(),\n        };')

# Same for NOT_FOUND, maybe keep LegacyApiError there, or use ApiError? The prompt says "ApiError" is standard for 400/413/415.
# Let's leave NOT_FOUND as LegacyApiError to not break existing clients if they check code "NOT_FOUND".
# Wait, we replaced ApiError -> LegacyApiError globally. So it's fine.

with open("src/api.rs", "w") as f:
    f.write(content)
