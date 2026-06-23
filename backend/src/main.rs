use std::net::SocketAddr;

use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wrong_question_notebook_rs::{
    AppConfig, AppState, build_router, connect_and_migrate, spawn_reaper,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // `admin` subcommand: bootstrap/recover super-admin accounts without
    // starting the HTTP server (e.g. `wqn-backend admin create --email ...`).
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("admin") {
        return wrong_question_notebook_rs::cli::run(&args[2..]).await;
    }

    init_tracing();

    let config = AppConfig::from_env().context("failed to load configuration")?;
    let pool = connect_and_migrate(&config).await?;

    let bind_addr: SocketAddr = config
        .bind_addr
        .parse()
        .with_context(|| format!("invalid APP_BIND_ADDR '{}'", config.bind_addr))?;

    let state = AppState::new(config, pool);
    // Reclaim orphaned problem-upload drafts left by abandoned forms. The
    // frontend's unmount cleanup is best-effort; this is the reliable兜底.
    spawn_reaper(state.clone());
    let app = build_router(state);

    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    tracing::info!(%bind_addr, "wrong-question-notebook-rs listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server failed")?;

    Ok(())
}

fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "wrong_question_notebook_rs=info,tower_http=info,sqlx=warn".into());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
