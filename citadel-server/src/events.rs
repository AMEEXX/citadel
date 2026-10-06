use std::convert::Infallible;
use std::time::Duration;
use axum::{
    extract::State,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Response},
};
use futures_util::stream::{self, StreamExt};
use crate::api::AppState;

pub async fn events_handler(State(state): State<AppState>) -> Response {
    let sse_enabled = std::env::var("CITADEL_NET_SSE")
        .map(|v| v != "0")
        .unwrap_or(true);

    if !sse_enabled {
        return StatusCode::NOT_FOUND.into_response();
    }

    // Task 26-T4.1: SSE bulkhead capacity limit
    let permit = match state.sse_slots.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "SSE connection limit reached",
            ).into_response();
        }
    };

    let rx = state.exam_tx.subscribe();
    let initial_val = (*state.exam_tx.borrow()).clone();
    let initial_json = serde_json::to_string(&*initial_val).unwrap_or_default();
    let initial_event = Ok::<_, Infallible>(Event::default().event("exam").data(initial_json));

    let mut ticker = tokio::time::interval(Duration::from_secs(30));
    ticker.tick().await; // consume initial immediate tick

    let event_stream = stream::unfold((rx, permit, ticker), |(mut rx, permit, mut ticker)| async move {
        tokio::select! {
            changed_res = rx.changed() => {
                if changed_res.is_err() {
                    return None;
                }
                let status = (*rx.borrow_and_update()).clone();
                let json_str = serde_json::to_string(&*status).unwrap_or_default();
                let event = Ok::<_, Infallible>(Event::default().event("exam").data(json_str));
                Some((event, (rx, permit, ticker)))
            }
            _ = ticker.tick() => {
                let status = (*rx.borrow()).clone();
                let json_str = serde_json::to_string(&*status).unwrap_or_default();
                let event = Ok::<_, Infallible>(Event::default().event("exam").data(json_str));
                Some((event, (rx, permit, ticker)))
            }
        }
    });

    let full_stream = stream::once(async move { initial_event }).chain(event_stream);

    let keep_alive = KeepAlive::new()
        .interval(Duration::from_secs(20))
        .text("");

    Sse::new(full_stream)
        .keep_alive(keep_alive)
        .into_response()
}
