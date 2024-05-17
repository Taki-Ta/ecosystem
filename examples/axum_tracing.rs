use std::time::Instant;

use anyhow::Ok;
use axum::{response::IntoResponse, routing::get, Router};
use tokio::{
    net::TcpListener,
    time::{self, error::Elapsed},
};
use tracing::{info, instrument, level_filters::LevelFilter, warn};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // tracing_subscriber::fmt::init();
    let console = fmt::Layer::new()
        .with_span_events(FmtSpan::CLOSE)
        .pretty()
        .with_filter(LevelFilter::DEBUG);
    let file_appender = tracing_appender::rolling::daily("tmp/logs", "ecosystem.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let file = fmt::Layer::new()
        .with_writer(non_blocking)
        .with_filter(LevelFilter::INFO);

    tracing_subscriber::registry()
        .with(console)
        .with(file)
        .init();
    let addr = "0.0.0.0:8081";
    let app = Router::new().route("/", get(index_handler));
    let listener = TcpListener::bind(addr).await?;
    info!("Server is running on: {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index_handler() -> impl IntoResponse {
    time::sleep(std::time::Duration::from_millis(50)).await;
    let res = long_task().await;
    res
}

#[instrument]
async fn long_task() -> &'static str {
    let start = Instant::now();
    time::sleep(std::time::Duration::from_millis(500)).await;
    let elapsed = start.elapsed().as_millis() as u64;
    warn!(app.task_duration = elapsed, "long task is done");
    "Hello, World!"
}
