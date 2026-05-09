use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;

use crate::device_service::config::load_config;
use crate::device_service::state::AppState;
use crate::device_service::types::{ApiError, RfidInfo};
use crate::device_service::utils::rfid::discovery::discover_rfid_readers;

#[derive(Serialize)]
struct RfidResponse {
    readers: Vec<RfidInfo>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct RfidWriteRequest {
    pub epc: String,
    pub data: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rfid", get(list))
        .route("/rfid/:name/stream", get(stream))
        .route("/rfid/:name/write", post(write))
}

async fn list() -> Json<RfidResponse> {
    let config = load_config();
    Json(RfidResponse {
        readers: discover_rfid_readers(&config),
    })
}

async fn stream(
    Path(name): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let config = load_config();
    if !config.rfid.contains_key(&name) {
        return not_found(&name);
    }
    let adapter = match state.rfid.get_or_build(&name, &config).await {
        Ok(a) => a,
        Err(e) => return rfid_error(StatusCode::NOT_IMPLEMENTED, "READER_INIT_FAILED", e.to_string()),
    };
    let rx = adapter.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(event) => {
                let json = serde_json::to_string(&event).ok()?;
                Some(Ok::<Event, Infallible>(Event::default().data(json)))
            }
            Err(_) => None,
        }
    });
    let initial = Event::default().data(format!(
        r#"{{"type":"open","message":"Streaming {name}"}}"#
    ));
    let combined: Box<dyn Stream<Item = Result<Event, Infallible>> + Send + Unpin> = Box::new(
        Box::pin(futures::stream::once(async move { Ok(initial) }).chain(stream)),
    );
    Sse::new(combined)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

async fn write(
    Path(name): Path<String>,
    State(_state): State<AppState>,
    Json(_body): Json<RfidWriteRequest>,
) -> Response {
    let config = load_config();
    if !config.rfid.contains_key(&name) {
        return not_found(&name);
    }
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(ApiError::new("NOT_IMPLEMENTED", "Tag write not wired yet.")),
    )
        .into_response()
}

fn not_found(name: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ApiError::new(
            "READER_NOT_FOUND",
            format!("RFID reader {name} not configured"),
        )),
    )
        .into_response()
}

fn rfid_error(status: StatusCode, code: &str, message: String) -> Response {
    (status, Json(ApiError::new(code, message))).into_response()
}
