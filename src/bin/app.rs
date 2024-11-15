use std::net::{Ipv4Addr, SocketAddr};

use adapter::database::connect_database_with;
use axum::Router;

use anyhow::{Context, Error, Result};
use registry::AppRegistry;
use shared::config::AppConfig;
use shared::env::{which, Environment};
use tokio::net::TcpListener;

use api::route::{book::build_book_routers, health::build_health_check_routers};

use tower_http::LatencyUnit;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use tower::http::LatencyUnit;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;

#[tokio::main]
async fn main() -> Result<()> {
    init_logger()?;
    bootstrap().await
}

fn init_logger() -> Result<()> {
    // Environment、環境によって出力するログレベルを変更する。本番環境ではinfo以上のログを出力する。ローカル環境ではdebug以上のログを出力する。
    let log_level = match which() {
        Environment::Development => "debug",
        Environment::Production => "info",
    };
    // 環境変数に設定されたログレベルを取得する。環境変数が設定されていない場合は、デフォルトのログレベルを取得する。
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| log_level.into());

    // ログのフォーマットを設定する。ファイル名、行番号、ターゲットを出力する。
    let subscriber = tracing_subscriber::fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_target(false);

    tracing_subscriber::registry()
        .with(subscriber)
        .with(env_filter)
        .try_init()?;
    Ok(())
}

async fn bootstrap() -> Result<()> {
    let app_config = AppConfig::new()?;
    let pool = connect_database_with(&app_config.database);

    let registry = AppRegistry::new(pool);
    let app = Router::new()
        .merge(build_health_check_routers())
        .merge(build_book_routers())
        .layer(cors())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Millis),
                ),
        )
        .with_state(registry);

    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8080);
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("LIstening on {}", addr);
    axum::serve(listener, app).await.context("Unexpected error happened in server").inspect_err(|e| {
        tracing::error!(error.cause_chain = ?e,error.message = %e,"Unexpected error happened in server");
    })
}
