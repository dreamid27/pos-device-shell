use std::net::SocketAddr;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};

use super::{config::load_config, routes, state::AppState};

pub async fn serve(default_port: u16) -> anyhow::Result<()> {
    let cfg = load_config();
    let port = if cfg.port != 0 { cfg.port } else { default_port };

    let state = AppState::new();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let stateful: Router<AppState> = Router::new()
        .merge(routes::rfid::router())
        .merge(routes::scales::router());

    let app: Router = Router::new()
        .merge(routes::health::router())
        .merge(routes::devices::router())
        .merge(routes::printers::router())
        .merge(routes::print::router())
        .merge(stateful.with_state(state))
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    log::info!("[device-service] listening on http://{addr}");

    axum::serve(listener, app).await?;
    Ok(())
}
