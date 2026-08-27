//! HTTP の経路と、全 handler で共有する状態を組み立てます。

use axum::{routing::get, Router};
use sqlx::SqlitePool;

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
        .with_state(AppState { pool })
}
