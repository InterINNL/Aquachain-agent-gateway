//! AquaChain agent gateway binary.

use std::sync::Arc;

use anyhow::Result;
use aquachain_agent_gateway::{build_router, routes::AppState, GatewayConfig, MeasurementStore};
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = GatewayConfig::from_env()?;
    config.validate_for_run()?;

    let state = AppState {
        config: Arc::new(config.clone()),
        store: Arc::new(MeasurementStore::default()),
    };

    let app = build_router(state).layer(TraceLayer::new_for_http()).layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    );

    let addr = config.bind_addr();
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(%addr, phase = "g1", "agent gateway listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
    tracing::info!("shutdown signal received");
}
