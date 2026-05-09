use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::get,
    Json, Router,
};
use futures::stream::{Stream, StreamExt};
use serde::Serialize;
use tokio_stream::wrappers::BroadcastStream;

use crate::device_service::config::load_config;
use crate::device_service::state::AppState;
use crate::device_service::types::{ApiError, ScaleInfo};
use crate::device_service::utils::scales::discovery::{build_catalog, discover_scales};

#[derive(Serialize)]
struct ScalesResponse {
    scales: Vec<ScaleInfo>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/scales", get(list))
        .route("/scales/:name/weight", get(weight))
        .route("/scales/:name/stream", get(stream))
}

async fn list() -> Json<ScalesResponse> {
    let config = load_config();
    let scales = tokio::task::spawn_blocking(move || discover_scales(&config))
        .await
        .unwrap_or_default();
    Json(ScalesResponse { scales })
}

async fn weight(Path(name): Path<String>, State(state): State<AppState>) -> Response {
    let config = load_config();
    if !catalog_has(&config, &name) {
        return not_found(&name);
    }
    let adapter = match state.scales.get_or_build(&name, &config).await {
        Ok(a) => a,
        Err(e) => return scale_error("SCALE_INIT_FAILED", e.to_string()),
    };
    match adapter.get_weight().await {
        Ok(reading) => Json(reading).into_response(),
        Err(e) => scale_error("SCALE_READ_FAILED", e.to_string()),
    }
}

async fn stream(
    Path(name): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let config = load_config();
    if !catalog_has(&config, &name) {
        return not_found(&name);
    }
    let adapter = match state.scales.get_or_build(&name, &config).await {
        Ok(a) => a,
        Err(e) => return scale_error("SCALE_INIT_FAILED", e.to_string()),
    };
    let rx = adapter.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(reading) => {
                let json = serde_json::to_string(&reading).ok()?;
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

fn catalog_has(config: &crate::device_service::types::Config, name: &str) -> bool {
    build_catalog(config).contains_key(name)
}

fn not_found(name: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ApiError::new(
            "SCALE_NOT_FOUND",
            format!("Scale {name} not detected or configured"),
        )),
    )
        .into_response()
}

fn scale_error(code: &str, message: String) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiError::new(code, message)),
    )
        .into_response()
}
