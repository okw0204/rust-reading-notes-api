use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use rust_reading_notes_api::{build_app, connect_database};
use serde_json::{Value, json};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tower::ServiceExt;

async fn test_app() -> axum::Router {
    test_app_and_pool().await.0
}

async fn test_app_and_pool() -> (axum::Router, SqlitePool) {
    // SQLite のインメモリ DB は接続ごとに別物なので、テストでは 1 接続に固定する。
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    (build_app(pool.clone()), pool)
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn database_connection_applies_migrations() {
    let pool = connect_database("sqlite::memory:").await.unwrap();

    let table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('books', 'notes')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(table_count, 2);
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

#[tokio::test]
async fn gets_a_book_with_its_notes() {
    let app = test_app().await;
    let create_request = Request::post("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Rust Atomics and Locks", "author": "Mara Bos"}).to_string(),
        ))
        .unwrap();
    app.clone().oneshot(create_request).await.unwrap();

    let response = app
        .oneshot(Request::get("/books/1").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await,
        json!({
            "id": 1,
            "title": "Rust Atomics and Locks",
            "author": "Mara Bos",
            "status": "want_to_read",
            "notes": []
        })
    );
}

#[tokio::test]
async fn returns_not_found_for_an_unknown_book() {
    let app = test_app().await;
    let response = app
        .oneshot(Request::get("/books/999").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        json_body(response).await,
        json!({
            "error": {
                "code": "not_found",
                "message": "resource not found"
            }
        })
    );
}

#[tokio::test]
async fn adds_a_note_to_a_book() {
    let app = test_app().await;
    let create_book = Request::post("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Rust Atomics and Locks", "author": "Mara Bos"}).to_string(),
        ))
        .unwrap();
    app.clone().oneshot(create_book).await.unwrap();

    let add_note = Request::post("/books/1/notes")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"body": "Ordering rules are the key."}).to_string(),
        ))
        .unwrap();
    let response = app.clone().oneshot(add_note).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        json_body(response).await,
        json!({"id": 1, "body": "Ordering rules are the key."})
    );

    let detail = app
        .oneshot(Request::get("/books/1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = json_body(detail).await;
    assert_eq!(
        body["notes"],
        json!([{"id": 1, "body": "Ordering rules are the key."}])
    );
}

#[tokio::test]
async fn rejects_a_blank_note() {
    let app = test_app().await;
    let create_book = Request::post("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Rust Atomics and Locks", "author": "Mara Bos"}).to_string(),
        ))
        .unwrap();
    app.clone().oneshot(create_book).await.unwrap();

    let request = Request::post("/books/1/notes")
        .header("content-type", "application/json")
        .body(Body::from(json!({"body": "  "}).to_string()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(response).await,
        json!({
            "error": {
                "code": "validation_error",
                "message": "note body must not be empty"
            }
        })
    );
}

#[tokio::test]
async fn returns_not_found_when_adding_a_note_to_an_unknown_book() {
    let app = test_app().await;
    let request = Request::post("/books/999/notes")
        .header("content-type", "application/json")
        .body(Body::from(json!({"body": "A note"}).to_string()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        json_body(response).await,
        json!({
            "error": {
                "code": "not_found",
                "message": "resource not found"
            }
        })
    );
}

#[tokio::test]
async fn updates_a_books_reading_status() {
    let app = test_app().await;
    let create_book = Request::post("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Programming Rust", "author": "Jim Blandy"}).to_string(),
        ))
        .unwrap();
    app.clone().oneshot(create_book).await.unwrap();

    let update = Request::patch("/books/1/status")
        .header("content-type", "application/json")
        .body(Body::from(json!({"status": "reading"}).to_string()))
        .unwrap();
    let response = app.clone().oneshot(update).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await,
        json!({
            "id": 1,
            "title": "Programming Rust",
            "author": "Jim Blandy",
            "status": "reading"
        })
    );

    let filtered = app
        .oneshot(
            Request::get("/books?status=reading")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json_body(filtered).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn rejects_an_unknown_reading_status_update() {
    let app = test_app().await;
    let request = Request::patch("/books/1/status")
        .header("content-type", "application/json")
        .body(Body::from(json!({"status": "paused"}).to_string()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();

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

#[tokio::test]
async fn returns_not_found_when_updating_an_unknown_book() {
    let app = test_app().await;
    let request = Request::patch("/books/999/status")
        .header("content-type", "application/json")
        .body(Body::from(json!({"status": "reading"}).to_string()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        json_body(response).await,
        json!({
            "error": {
                "code": "not_found",
                "message": "resource not found"
            }
        })
    );
}

#[tokio::test]
async fn deletes_a_book_and_its_notes() {
    let (app, pool) = test_app_and_pool().await;
    let create_book = Request::post("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Rust Atomics and Locks", "author": "Mara Bos"}).to_string(),
        ))
        .unwrap();
    app.clone().oneshot(create_book).await.unwrap();
    let add_note = Request::post("/books/1/notes")
        .header("content-type", "application/json")
        .body(Body::from(json!({"body": "A note"}).to_string()))
        .unwrap();
    app.clone().oneshot(add_note).await.unwrap();

    let response = app
        .oneshot(Request::delete("/books/1").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .len(),
        0
    );
    let note_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(note_count, 0);
}

#[tokio::test]
async fn returns_not_found_when_deleting_an_unknown_book() {
    let app = test_app().await;
    let response = app
        .oneshot(Request::delete("/books/999").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        json_body(response).await,
        json!({
            "error": {
                "code": "not_found",
                "message": "resource not found"
            }
        })
    );
}
