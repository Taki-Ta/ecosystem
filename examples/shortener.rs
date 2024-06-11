use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::{header::LOCATION, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use tokio::net::TcpListener;
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

#[derive(Debug, Deserialize, Serialize)]
struct ShortenReq {
    url: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ShortenRsp {
    url: String,
}

#[derive(Debug, Clone)]
struct AppState {
    db: PgPool,
}

#[derive(Debug, FromRow)]
struct UrlRecord {
    #[sqlx(default)]
    id: String,
    #[sqlx(default)]
    url: String,
}

const CONNECT_URL: &str = "postgrJsones://postgres:postgres@localhost:5432/shortener";
const LISTEN_ADDR: &str = "127.0.0.1:9876";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let layer = Layer::new().pretty().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();
    let listener = TcpListener::bind(LISTEN_ADDR).await?;
    let state = AppState::try_new(CONNECT_URL).await?;
    info!("Connected to database: {CONNECT_URL}");
    info!("Listening on {LISTEN_ADDR}");

    let app = Router::new()
        .route("/", post(shorten))
        .route("/:id", get(redirect))
        .with_state(state);
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

async fn shorten(
    State(state): State<AppState>,
    Json(data): Json<ShortenReq>,
) -> Result<impl IntoResponse, StatusCode> {
    info!("get request, url is {}", &data.url);
    let id = state.shorten(&data.url).await.map_err(|e| {
        warn!("Failed to shorten URL: {e}");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;
    let body = Json(ShortenRsp {
        url: format!("http://{}/{}", LISTEN_ADDR, id),
    });
    Ok((StatusCode::CREATED, body))
}

async fn redirect(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    info!("get request, id is {}", &id);
    let url = state
        .get_url(&id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, url.parse().unwrap());
    Ok((StatusCode::PERMANENT_REDIRECT, headers))
}

impl AppState {
    async fn try_new(url: &str) -> anyhow::Result<Self> {
        let pool = PgPool::connect(url).await?;
        sqlx::query(
            r#"
                create table if not exists urls(
                    id char(6) primary key,
                    url text not null unique
                )
            "#,
        )
        .execute(&pool)
        .await?;
        Ok(Self { db: pool })
    }

    async fn shorten(&self, url: &str) -> Result<String> {
        let id = "A4FS5Q".to_string();
        let sql="INSERT INTO urls (id, url) VALUES ($1, $2) ON CONFLICT(url) DO UPDATE SET url=EXCLUDED.url RETURNING id";
        let Ok(ret): Result<UrlRecord, sqlx::Error> = sqlx::query_as(sql)
            .bind(&id)
            .bind(url)
            .fetch_one(&self.db)
            .await
        else {
            return Err(Errors::ExecuteError(sql.to_string()).into());
        };
        Ok(ret.id)
    }

    async fn get_url(&self, id: &str) -> anyhow::Result<String, Errors> {
        // let ret = sqlx::query_as("SELECT url FROM urls WHERE id = $1")
        //     .bind(id)
        //     .fetch_one(&self.db)
        //     .await;
        let sql = "SELECT url FROM urls WHERE id = $1";
        let Ok(ret): Result<UrlRecord, sqlx::Error> =
            sqlx::query_as(sql).bind(id).fetch_one(&self.db).await
        else {
            return Err(Errors::ExecuteError(sql.to_string()));
        };
        Ok(ret.url)
    }
}

#[derive(Error, Debug)]
enum Errors {
    // #[error("Can not connect to database:{0}")]
    // ConnectedError(String),
    // #[error("Can not get redirect url:{0}")]
    // RedirectError(String),
    #[error("Sql executed error:{0}")]
    ExecuteError(String),
}

impl IntoResponse for Errors {
    fn into_response(self) -> Response {
        match self {
            // Errors::ConnectedError(err)=>err.into_response(),
            // Errors::RedirectError(err)=>err.into_response(),
            Errors::ExecuteError(err) => err.into_response(),
        }
    }
}
