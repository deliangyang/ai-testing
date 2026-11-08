use std::convert::Infallible;
use std::time::Duration;

use async_stream::stream;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use engine::EngineEvent;
use futures_util::Stream;
use tokio::sync::broadcast::error::RecvError;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct EventsQuery {
    pub since: Option<u64>,
}

pub async fn run_event_stream(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<EventsQuery>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let since = last_event_id(&headers).or(query.since).unwrap_or(0);

    let state_check = state.clone();
    let rid = run_id.clone();
    tokio::task::spawn_blocking(move || {
        state_check.with_db(|db| {
            let repo = store::RunRepository::new(
                db.conn(),
                state_check.clock.as_ref(),
                state_check.ids.as_ref(),
            );
            repo.get(&rid)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))??
    .ok_or_else(|| ApiError::NotFound(format!("run '{run_id}'")))?;

    let hub = state.hub.clone();
    let run_id_live = run_id.clone();

    let stream = stream! {
        let mut cursor = since;
        let mut finished = false;
        for item in hub.replay(&run_id, cursor) {
            if let Some(event) = to_sse(&item.event, item.seq) {
                cursor = item.seq;
                if matches!(item.event, EngineEvent::RunFinished { .. }) {
                    finished = true;
                }
                yield Ok(event);
            }
        }
        if finished {
            return;
        }
        let mut live = hub.subscribe(&run_id);
        loop {
            match live.recv().await {
                Ok(item) if item.seq > cursor => {
                    if let Some(event) = to_sse(&item.event, item.seq) {
                        cursor = item.seq;
                        yield Ok(event);
                        if matches!(item.event, EngineEvent::RunFinished { .. }) {
                            break;
                        }
                    }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => {
                    for item in hub.replay(&run_id_live, cursor) {
                        if let Some(event) = to_sse(&item.event, item.seq) {
                            cursor = item.seq;
                            yield Ok(event);
                        }
                    }
                }
                Err(RecvError::Closed) => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

fn last_event_id(headers: &HeaderMap) -> Option<u64> {
    headers
        .get("last-event-id")
        .or_else(|| headers.get("Last-Event-ID"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
}

fn to_sse(event: &EngineEvent, seq: u64) -> Option<Event> {
    Event::default()
        .id(seq.to_string())
        .event(event_type(event))
        .json_data(event)
        .ok()
}

fn event_type(event: &EngineEvent) -> &'static str {
    match event {
        EngineEvent::NodeStateChanged { .. } => "node_state_changed",
        EngineEvent::PollIteration { .. } => "poll_iteration",
        EngineEvent::Log { .. } => "log",
        EngineEvent::RunFinished { .. } => "run_finished",
    }
}
