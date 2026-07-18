use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock as TokioRwLock;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};
use uuid::Uuid;

// Types
#[derive(Debug, Serialize, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)] // response payload; fields serialized, not read
struct UserInfo {
    username: String,
    #[allow(dead_code)] // serialized for clients; not read server-side
    authenticated_at: DateTime<Utc>,
    session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    timestamp: DateTime<Utc>,
    service: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatusResponse {
    status: String,
    active_sessions: usize,
    timestamp: DateTime<Utc>,
    service: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct HelloResponse {
    message: String,
    authenticated: bool,
    session_id: Option<String>,
    timestamp: DateTime<Utc>,
    service: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PublicResponse {
    message: String,
    authenticated: bool,
    timestamp: DateTime<Utc>,
    service: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct LogoutResponse {
    message: String,
}

// Session storage
type Sessions = Arc<TokioRwLock<HashMap<String, SessionData>>>;

#[derive(Debug, Clone)]
struct SessionData {
    username: String,
    session_id: String,
    #[allow(dead_code)] // recorded for auditability; not read yet
    authenticated_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

#[derive(Clone)]
struct AppState {
    sessions: Sessions,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create app state
    let state = AppState {
        sessions: Arc::new(TokioRwLock::new(HashMap::new())),
    };

    // Create router
    let app = Router::new()
        .route("/", get(health_check))
        .route("/public", get(public_endpoint))
        .route("/status", get(status_endpoint))
        .route("/api/test", get(api_test))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/hello", get(hello_authenticated))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Get port from environment or default
    let bind_address =
        std::env::var("SERVICE_BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8001".to_string());

    let parts: Vec<&str> = bind_address.split(':').collect();
    let host = parts.first().unwrap_or(&"0.0.0.0");
    let port = parts
        .get(1)
        .unwrap_or(&"8001")
        .parse::<u16>()
        .unwrap_or(8001);

    info!("Starting FleetingDNS Test Service on {}:{}", host, port);

    // Start server
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port))
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> Json<HealthResponse> {
    info!("Health check requested");
    Json(HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
        service: "fleetingdns-test-service".to_string(),
    })
}

async fn public_endpoint() -> Json<PublicResponse> {
    info!("Public endpoint accessed");
    Json(PublicResponse {
        message: "This is a public endpoint".to_string(),
        authenticated: false,
        timestamp: Utc::now(),
        service: "fleetingdns-test-service".to_string(),
    })
}

async fn api_test() -> Json<HelloResponse> {
    info!("API test endpoint accessed");
    Json(HelloResponse {
        message: "Hello from FleetingDNS Test Service!".to_string(),
        authenticated: false,
        session_id: None,
        timestamp: Utc::now(),
        service: "fleetingdns-test-service".to_string(),
    })
}

async fn status_endpoint(State(state): State<AppState>) -> Json<StatusResponse> {
    let sessions = state.sessions.read().await;
    let active_sessions = sessions.len();
    info!("Status check - active sessions: {}", active_sessions);

    Json(StatusResponse {
        status: "running".to_string(),
        active_sessions,
        timestamp: Utc::now(),
        service: "fleetingdns-test-service".to_string(),
    })
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    info!("Login attempt for user: {}", payload.username);

    // Simple authentication (in production, validate against database)
    if payload.username == "testuser" && payload.password == "testpass" {
        // Generate token and session
        let token = Uuid::new_v4().to_string();
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + chrono::Duration::hours(1);

        let session_data = SessionData {
            username: payload.username.clone(),
            session_id: session_id.clone(),
            authenticated_at: now,
            expires_at,
        };

        // Store session
        state
            .sessions
            .write()
            .await
            .insert(token.clone(), session_data);

        info!("Login successful for user: {}", payload.username);
        Ok(Json(LoginResponse {
            access_token: token,
            token_type: "bearer".to_string(),
            expires_in: 3600,
        }))
    } else {
        warn!("Login failed for user: {}", payload.username);
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LogoutResponse>, StatusCode> {
    let token = extract_token(&headers)?;

    // Find and remove session
    let mut sessions = state.sessions.write().await;
    if let Some(session) = sessions.remove(&token) {
        info!("Session invalidated for user: {}", session.username);
    }

    Ok(Json(LogoutResponse {
        message: "Logged out successfully".to_string(),
    }))
}

async fn hello_authenticated(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<HelloResponse>, StatusCode> {
    let token = extract_token(&headers)?;

    // Validate session
    let sessions = state.sessions.read().await;
    let session = sessions.get(&token).ok_or(StatusCode::UNAUTHORIZED)?;

    if Utc::now() > session.expires_at {
        // Session expired
        return Err(StatusCode::UNAUTHORIZED);
    }

    info!(
        "Authenticated hello requested by user: {}",
        session.username
    );

    Ok(Json(HelloResponse {
        message: format!("Hello, {}!", session.username),
        authenticated: true,
        session_id: Some(session.session_id.clone()),
        timestamp: Utc::now(),
        service: "fleetingdns-test-service".to_string(),
    }))
}

fn extract_token(headers: &HeaderMap) -> Result<String, StatusCode> {
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(auth_header[7..].to_string())
}
