use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rust_reading_notes_api::build_app;
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

async fn test_app() -> axum::Router {
    // SQLite のインメモリ DB は接続ごとに別物なので、テストでは 1 接続に固定する。
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    build_app(pool)
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn creates_a_book() {
    let app = test_app().await;
    let request = Request::post("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Rust for Rustaceans", "author": "Jon Gjengset"}).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        json_body(response).await,
        json!({
            "id": 1,
            "title": "Rust for Rustaceans",
            "author": "Jon Gjengset",
            "status": "want_to_read"
        })
    );
}

#[tokio::test]
async fn rejects_a_blank_title() {
    let app = test_app().await;
    let request = Request::post("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "   ", "author": "Jon Gjengset"}).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(response).await,
        json!({
            "error": {
                "code": "validation_error",
                "message": "title must not be empty"
            }
        })
    );
}

#[tokio::test]
async fn rejects_a_blank_author() {
    let app = test_app().await;
    let request = Request::post("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Rust for Rustaceans", "author": "\t"}).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(response).await,
        json!({
            "error": {
                "code": "validation_error",
                "message": "author must not be empty"
            }
        })
    );
}

#[tokio::test]
async fn lists_books() {
    let app = test_app().await;
    let create_request = Request::post("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Programming Rust", "author": "Jim Blandy"}).to_string(),
        ))
        .unwrap();
    app.clone().oneshot(create_request).await.unwrap();

    let response = app
        .oneshot(Request::get("/books").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await,
        json!([{
            "id": 1,
            "title": "Programming Rust",
            "author": "Jim Blandy",
            "status": "want_to_read"
        }])
    );
}

#[tokio::test]
async fn filters_books_by_reading_status() {
    let app = test_app().await;
    let create_request = Request::post("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Programming Rust", "author": "Jim Blandy"}).to_string(),
        ))
        .unwrap();
    app.clone().oneshot(create_request).await.unwrap();

    let response = app
        .oneshot(
            Request::get("/books?status=finished")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await, json!([]));
}

#[tokio::test]
async fn rejects_an_unknown_reading_status_filter() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::get("/books?status=paused")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(response).await,
        json!({
            "error": {
                "code": "validation_error",
                "message": "status must be one of: want_to_read, reading, finished"
            }
        })
    );
}
