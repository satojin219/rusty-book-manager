use shared::{
    config::DatabaseConfig,
    error::{AppError, AppResult},
};
use sqlx::postgres::{PgConnectOptions, PgPool};

// DatabaseCOnfigからPgCOnnectOptionsに変換する関数
fn make_pg_connect_options(cfg: &DatabaseConfig) -> PgConnectOptions {
    PgConnectOptions::new()
        .host(&cfg.host)
        .port(cfg.port)
        .username(&cfg.username)
        .password(&cfg.password)
        .database(&cfg.database)
}

#[derive(Clone)]
pub struct ConnectionPool(PgPool);
pub mod model;

impl ConnectionPool {
    pub fn new(pool: PgPool) -> Self {
        Self(pool)
    }
    //sqlx::PgPoolへの参照を取得する
    pub fn inner_ref(&self) -> &PgPool {
        &self.0
    }

    pub async fn begin(&self) -> AppResult<sqlx::Transaction<'_, sqlx::Postgres>> {
        self.0.begin().await.map_err(AppError::TransactionError)
    }
}

pub fn connect_database_with(cfg: &DatabaseConfig) -> ConnectionPool {
    ConnectionPool(PgPool::connect_lazy_with(make_pg_connect_options(cfg)))
}
