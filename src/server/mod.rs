pub mod auth;
pub mod error;
pub mod routes;
pub mod state;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::server::error::fallback_404;
use crate::server::routes::auth::{local_mode_handler, local_session_handler, me_handler};
use crate::server::routes::health::health_handler;
use crate::server::state::AppState;

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/auth/local-mode", get(local_mode_handler))
        .route("/api/v1/auth/local-session", post(local_session_handler))
        .route("/api/v1/auth/me", get(me_handler))
        .fallback(fallback_404)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn serve_app(
    state: AppState,
    listener: tokio::net::TcpListener,
) -> Result<(), std::io::Error> {
    let app = create_app(state);
    axum::serve(listener, app).await
}
