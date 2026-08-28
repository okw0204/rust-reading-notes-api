//! HTTP の経路と、全 handler で共有する状態を組み立てます。

use axum::{Router, routing::get};
use std::{error::Error, str::FromStr};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::handler;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) pool: SqlitePool,
}

/// production と統合テストで共通利用する Router を構築します。
pub fn build_app(pool: SqlitePool) -> Router {
    Router::new()
        .route(
            "/books",
            get(handler::list_books).post(handler::create_book),
        )
        .route(
            "/books/{id}",
            get(handler::get_book).delete(handler::delete_book),
        )
        .route(
            "/books/{id}/status",
            axum::routing::patch(handler::update_status),
        )
        .route("/books/{id}/notes", axum::routing::post(handler::add_note))
        .with_state(AppState { pool })
}

/// SQLite へ接続し、未適用の migration を実行します。
pub async fn connect_database(url: &str) -> Result<SqlitePool, Box<dyn Error>> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true);
    // 教材では単純な共有状態を優先する。1 接続ならインメモリ DB も同じ経路でテストできる。
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}
