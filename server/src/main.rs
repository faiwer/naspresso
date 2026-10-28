use std::sync::Arc;

use anyhow::Context;
use config::Config;
use state::AppState;
use tokio::{net::TcpListener, signal};

mod config;
mod error;
mod routes;
mod state;
mod storage;
mod telemetry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Before telemetry, so `RUST_LOG` from `.env` takes effect.
    let config = Config::from_env().context("loading config from environment")?;

    telemetry::init();

    let (host, port) = (config.host.as_str(), config.port);
    let listener = TcpListener::bind((host, port))
        .await
        .with_context(|| format!("binding to {host}:{port}"))?;

    let local_addr = listener.local_addr().context("reading local addr")?;
    tracing::info!(%local_addr, storage = %config.storage.display(), "server listening");

    let state = AppState {
        config: Arc::new(config),
    };
    axum::serve(listener, routes::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    tracing::info!("server stopped cleanly");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received Ctrl-C, shutting down"),
        _ = terminate => tracing::info!("received SIGTERM, shutting down"),
    }
}
