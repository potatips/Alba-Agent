use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use std::collections::HashSet;
use std::convert::Infallible;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

use crate::events::types::RuntimeEventType;
use crate::server::auth::AuthenticatedUser;
use crate::server::error::ApiError;
use crate::server::state::AppState;

pub async fn stream_run(
    Path(id): Path<String>,
    headers: HeaderMap,
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let target_run_id = Uuid::from_str(&id).map_err(|_| ApiError::not_found("Run not found"))?;

    let run_record = state
        .store
        .get_run(&id)
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::not_found("Run not found"))?;

    let last_event_id = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let mut rx = state.events_tx.subscribe();
    let store = state.store.clone();
    let run_id_str = id.clone();
    let initial_status = run_record.status;

    let stream = async_stream::stream! {
        let mut emitted: HashSet<Uuid> = HashSet::new();
        let mut last_id_emitted: Option<String> = None;

        let historical = store
            .list_run_events(&run_id_str, last_event_id.as_deref())
            .unwrap_or_default();

        let mut terminal_reached = false;
        for event in historical {
            emitted.insert(event.id);
            last_id_emitted = Some(event.id.to_string());
            let ev_type = event.event_type;
            if let Ok(data) = serde_json::to_string(&event) {
                let sse_ev = Event::default()
                    .id(event.id.to_string())
                    .event(ev_type.as_str())
                    .data(data);
                yield Ok(sse_ev);
            }
            if ev_type == RuntimeEventType::RunCompleted || ev_type == RuntimeEventType::RunFailed {
                terminal_reached = true;
                break;
            }
        }

        if terminal_reached {
            return;
        }

        if initial_status == "SUCCEEDED"
            || initial_status == "FAILED"
            || initial_status == "CANCELLED"
        {
            return;
        }

        loop {
            tokio::select! {
                res = rx.recv() => {
                    match res {
                        Ok(event) => {
                            if event.scope.run_id == target_run_id && !emitted.contains(&event.id) {
                                emitted.insert(event.id);
                                last_id_emitted = Some(event.id.to_string());
                                let ev_type = event.event_type;
                                if let Ok(data) = serde_json::to_string(&event) {
                                    let sse_ev = Event::default()
                                        .id(event.id.to_string())
                                        .event(ev_type.as_str())
                                        .data(data);
                                    yield Ok(sse_ev);
                                }
                                if ev_type == RuntimeEventType::RunCompleted || ev_type == RuntimeEventType::RunFailed {
                                    break;
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            let missed = store
                                .list_run_events(&run_id_str, last_id_emitted.as_deref())
                                .unwrap_or_default();
                            let mut lagged_terminal = false;
                            for event in missed {
                                if !emitted.contains(&event.id) {
                                    emitted.insert(event.id);
                                    last_id_emitted = Some(event.id.to_string());
                                    let ev_type = event.event_type;
                                    if let Ok(data) = serde_json::to_string(&event) {
                                        let sse_ev = Event::default()
                                            .id(event.id.to_string())
                                            .event(ev_type.as_str())
                                            .data(data);
                                        yield Ok(sse_ev);
                                    }
                                    if ev_type == RuntimeEventType::RunCompleted || ev_type == RuntimeEventType::RunFailed {
                                        lagged_terminal = true;
                                        break;
                                    }
                                }
                            }
                            if lagged_terminal {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}
