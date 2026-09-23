//! HTTP の経路と、全 handler で共有する状態を組み立てます。

use axum::{Router, routing::get};
use std::{error::Error, str::FromStr, sync::Arc};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::{
    handler,
    repository::{BookRepository, SqliteBookRepository},
    service::ReadingService,
};

// ANCHOR: composition
pub(crate) struct AppState<R> {
    // Arc は service の共有所有、repository 内の pool は DB 接続の共有を担う。
    pub(crate) service: Arc<ReadingService<R>>,
}

impl<R> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            service: Arc::clone(&self.service),
        }
    }
}

/// SQLite Adapter を使う production と統合テスト向けの Router を構築します。
pub fn build_app(pool: SqlitePool) -> Router {
    build_app_with_repository(SqliteBookRepository::new(pool))
}

/// 指定された永続化 Adapter で Router を構築します。
pub(crate) fn build_app_with_repository<R>(repository: R) -> Router
where
    R: BookRepository + 'static,
{
    let state = AppState {
        service: Arc::new(ReadingService::new(repository)),
    };
    Router::new()
        .route(
            "/books",
            get(handler::list_books::<R>).post(handler::create_book::<R>),
        )
        .route(
            "/books/{id}",
            get(handler::get_book::<R>).delete(handler::delete_book::<R>),
        )
        .route(
            "/books/{id}/status",
            axum::routing::patch(handler::update_status::<R>),
        )
        .route(
            "/books/{id}/notes",
            axum::routing::post(handler::add_note::<R>),
        )
        .with_state(state)
}

// ANCHOR_END: composition

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

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::build_app_with_repository;
    use crate::test_support::FakeBookRepository;

    async fn json_body(response: axum::response::Response) -> Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn builds_router_with_supplied_repository_adapter() {
        let app = build_app_with_repository(FakeBookRepository::default());
        let response = app
            .clone()
            .oneshot(
                Request::post("/books")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"title":"Rust Book","author":"Author"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            json_body(response).await,
            json!({
                "id": 1,
                "title": "Rust Book",
                "author": "Author",
                "status": "want_to_read"
            })
        );

        let response = app
            .oneshot(Request::get("/books/1").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            json_body(response).await,
            json!({
                "id": 1,
                "title": "Rust Book",
                "author": "Author",
                "status": "want_to_read",
                "notes": []
            })
        );
    }
}
