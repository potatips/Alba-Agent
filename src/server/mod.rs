pub mod auth;
pub mod error;
pub mod routes;
pub mod state;
pub mod workflow_runner;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::server::error::fallback_404;
use crate::server::routes::approvals::{list_approvals_handler, resolve_approval_handler};
use crate::server::routes::auth::{local_mode_handler, local_session_handler, me_handler};
use crate::server::routes::capsules::list_capsules_handler;
use crate::server::routes::health::health_handler;
use crate::server::routes::runs::{
    cancel_run_handler, fork_run_checkpoint_handler, get_run_handler, list_run_artifacts_handler,
    list_run_checkpoints_handler, list_run_evidence_handler, list_runs_handler, resume_run_handler,
    rewind_run_checkpoint_handler,
};
use crate::server::routes::settings::{
    get_chat_models_handler, get_execution_policy_handler, update_execution_policy_handler,
};
use crate::server::routes::sse::stream_run;
use crate::server::routes::tools::{list_builtin_tools_handler, list_skills_handler};
use crate::server::routes::workflows::{
    create_workflow_handler, create_workflow_run_handler, get_workflow_handler,
    import_workflow_handler, list_conversations_handler, list_templates_handler,
    list_workflows_handler, update_workflow_handler, validate_workflow_handler,
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
        .route(
            "/api/v1/workflows/validate",
            post(validate_workflow_handler),
        )
        .route("/api/v1/workflows/import", post(import_workflow_handler))
        .route("/api/v1/conversations", get(list_conversations_handler))
        .route("/api/v1/templates", get(list_templates_handler))
        .route("/api/v1/runs", get(list_runs_handler))
        .route("/api/v1/runs/{id}", get(get_run_handler))
        .route("/api/v1/runs/{id}/stream", get(stream_run))
        .route("/api/v1/runs/{id}/cancel", post(cancel_run_handler))
        .route("/api/v1/runs/{id}/resume", post(resume_run_handler))
        .route(
            "/api/v1/runs/{id}/checkpoints",
            get(list_run_checkpoints_handler),
        )
        .route(
            "/api/v1/runs/{id}/checkpoints/{chk_id}/fork",
            post(fork_run_checkpoint_handler),
        )
        .route(
            "/api/v1/runs/{id}/checkpoints/{chk_id}/rewind",
            post(rewind_run_checkpoint_handler),
        )
        .route(
            "/api/v1/runs/{id}/artifacts",
            get(list_run_artifacts_handler),
        )
        .route("/api/v1/runs/{id}/evidence", get(list_run_evidence_handler))
        .route("/api/v1/approvals", get(list_approvals_handler))
        .route(
            "/api/v1/approvals/{id}/resolve",
            post(resolve_approval_handler),
        )
        .route(
            "/api/v1/learning-transfer/capsules",
            get(list_capsules_handler),
        )
        .route("/api/v1/capsules/search", get(list_capsules_handler))
        .route("/api/v1/settings/chat-models", get(get_chat_models_handler))
        .route(
            "/api/v1/settings/execution",
            get(get_execution_policy_handler).put(update_execution_policy_handler),
        )
        .route(
            "/api/v1/settings/execution-policy",
            get(get_execution_policy_handler).put(update_execution_policy_handler),
        )
        .route("/api/v1/tools/builtin", get(list_builtin_tools_handler))
        .route("/api/v1/skills", get(list_skills_handler))
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
