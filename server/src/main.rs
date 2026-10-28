use anyhow::Context;
use config::Config;
use tokio::{net::TcpListener, signal};

mod config;
mod routes;
mod telemetry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Before telemetry, so `RUST_LOG` from `.env` takes effect.
    let config = Config::from_env().context("loading config from environment")?;

    telemetry::init();

    let Config { host, port } = &config;
    let listener = TcpListener::bind((host.as_str(), *port))
        .await
        .with_context(|| format!("binding to {host}:{port}"))?;

    let local_addr = listener.local_addr().context("reading local addr")?;
    tracing::info!(%local_addr, "server listening");

    axum::serve(listener, routes::router())
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
