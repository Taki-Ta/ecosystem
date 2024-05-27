use axum::{extract::Request, response::IntoResponse, routing::get, Router};
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    runtime,
    trace::{self, RandomIdGenerator, Tracer},
    Resource,
};
use std::time::Duration;
use std::time::Instant;
use tokio::{
    join,
    net::TcpListener,
    time::{self, sleep},
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
    //console layer
    let console = fmt::Layer::new()
        .with_span_events(FmtSpan::CLOSE)
        .pretty()
        .with_filter(LevelFilter::DEBUG);

    //file layer
    let file_appender = tracing_appender::rolling::daily("tmp/logs", "ecosystem.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let file = fmt::Layer::new()
        .with_writer(non_blocking)
        .with_filter(LevelFilter::INFO);

    //open telemetry layer
    let tracer = init_tracer()?;
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(console)
        .with(file)
        .with(opentelemetry)
        .init();

    let addr = "0.0.0.0:8080";
    let app = Router::new().route("/", get(index_handler));
    let listener = TcpListener::bind(addr).await?;
    info!("Server is running on: {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

#[instrument(fields(http.uri = req.uri().path(), http.method = req.method().as_str()))]
async fn index_handler(req: Request) -> impl IntoResponse {
    time::sleep(std::time::Duration::from_millis(50)).await;
    let res = long_task().await;
    info!(http.status_code = 200, "index handler completed");
    res
}

#[instrument]
async fn long_task() -> &'static str {
    let start = Instant::now();
    let s1 = time::sleep(std::time::Duration::from_millis(500));

    let t1 = task1();
    let t2 = task2();
    let t3 = task3();
    join!(s1, t1, t2, t3);
    let elapsed = start.elapsed().as_millis() as u64;
    warn!(app.task_duration = elapsed, "long task is done");
    "Hello, World!"
}

fn init_tracer() -> anyhow::Result<Tracer> {
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317"),
        )
        .with_trace_config(
            trace::config()
                .with_id_generator(RandomIdGenerator::default())
                .with_max_events_per_span(32)
                .with_max_attributes_per_span(64)
                .with_resource(Resource::new(vec![KeyValue::new(
                    "service.name",
                    "axum_tracing",
                )])),
        )
        .install_batch(runtime::Tokio)?;
    Ok(tracer)
}

#[instrument]
async fn task1() {
    sleep(Duration::from_millis(10)).await;
}

#[instrument]
async fn task2() {
    sleep(Duration::from_millis(50)).await;
}

#[instrument]
async fn task3() {
    sleep(Duration::from_millis(30)).await;
}
