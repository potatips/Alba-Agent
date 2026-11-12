pub mod auth;
pub mod error;
pub mod routes;
pub mod state;
pub mod workflow_runner;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::server::error::fallback_404;
use crate::server::routes::auth::{local_mode_handler, local_session_handler, me_handler};
use crate::server::routes::health::health_handler;
use crate::server::routes::runs::{get_run_handler, list_runs_handler};
use crate::server::routes::sse::stream_run;
use crate::server::routes::workflows::{
    create_workflow_handler, create_workflow_run_handler, get_workflow_handler,
    list_workflows_handler, update_workflow_handler,
};
use crate::server::state::AppState;

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/auth/local-mode", get(local_mode_handler))
        .route("/api/v1/auth/local-session", post(local_session_handler))
        .route("/api/v1/auth/me", get(me_handler))
        .route(
            "/api/v1/workflows",
            get(list_workflows_handler).post(create_workflow_handler),
        )
        .route(
            "/api/v1/workflows/{id}",
            get(get_workflow_handler).put(update_workflow_handler),
        )
        .route(
            "/api/v1/workflows/{id}/runs",
            post(create_workflow_run_handler),
        )
        .route("/api/v1/runs", get(list_runs_handler))
        .route("/api/v1/runs/{id}", get(get_run_handler))
        .route("/api/v1/runs/{id}/stream", get(stream_run))
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
