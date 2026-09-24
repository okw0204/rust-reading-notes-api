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

// ANCHOR: api_test_setup
async fn test_app_and_pool() -> (axum::Router, SqlitePool) {
    // 教材の接続管理を単純にするため最大 1 接続にする。各テストは別の pool を作る。
    // SQLx 0.8.6 の sqlite::memory: は、同じ pool の複数接続でも DB を共有できる。
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

// ANCHOR_END: api_test_setup

fn json_request(method: &str, path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn hides_database_error_details() {
    let (app, pool) = test_app_and_pool().await;
    pool.close().await;
    let response = app
        .oneshot(Request::get("/books").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        json_body(response).await,
        json!({"error": {"code": "internal_error", "message": "internal server error"}})
    );
}

#[tokio::test]
async fn hides_invalid_stored_value_details() {
    let (app, pool) = test_app_and_pool().await;
    sqlx::query(
        "INSERT INTO books (id, title, author, status) VALUES (1, ?, 'Author', 'want_to_read')",
    )
    .bind(" \t\n")
    .execute(&pool)
    .await
    .unwrap();
    for path in ["/books/1", "/books"] {
        let response = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            json_body(response).await,
            json!({"error": {"code": "internal_error", "message": "internal server error"}})
        );
    }
}

#[tokio::test]
async fn rejects_a_non_numeric_book_path() {
    let app = test_app().await;
    let response = app
        .oneshot(
            Request::get("/books/not-a-number")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Path の rejection は Axum が返すため、アプリケーションの JSON 形式を要求しない。
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_skipping_the_reading_state() {
    let app = test_app().await;
    let created = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/books",
            json!({"title":"Rust Book","author":"Author"}),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let response = app
        .clone()
        .oneshot(json_request(
            "PATCH",
            "/books/1/status",
            json!({"status":"finished"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_body(response).await,
        json!({"error": {
            "code": "conflict", "message": "reading state conflict"
        }})
    );
    let response = app
        .oneshot(Request::get("/books/1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(json_body(response).await["status"], "want_to_read");
}

#[tokio::test]
async fn accepts_only_forward_adjacent_transitions() {
    let states = ["want_to_read", "reading", "finished"];
    for (from, &current) in states.iter().enumerate() {
        for (to, &requested) in states.iter().enumerate() {
            let app = test_app().await;
            let created = app
                .clone()
                .oneshot(json_request(
                    "POST",
                    "/books",
                    json!({"title":"Title","author":"Author"}),
                ))
                .await
                .unwrap();
            assert_eq!(created.status(), StatusCode::CREATED);
            for &step in states.iter().take(from + 1).skip(1) {
                let response = app
                    .clone()
                    .oneshot(json_request(
                        "PATCH",
                        "/books/1/status",
                        json!({"status":step}),
                    ))
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
            }
            let response = app
                .clone()
                .oneshot(json_request(
                    "PATCH",
                    "/books/1/status",
                    json!({"status":requested}),
                ))
                .await
                .unwrap();
            let allowed = to == from + 1;
            assert_eq!(
                response.status(),
                if allowed {
                    StatusCode::OK
                } else {
                    StatusCode::CONFLICT
                },
                "{current} -> {requested}"
            );
            if !allowed {
                assert_eq!(json_body(response).await["error"]["code"], "conflict");
            }
            let stored = app
                .oneshot(Request::get("/books/1").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(stored.status(), StatusCode::OK);
            assert_eq!(
                json_body(stored).await["status"],
                if allowed { requested } else { current }
            );
        }
    }
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

// ANCHOR: api_delete_test
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

// ANCHOR_END: api_delete_test

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

#[tokio::test]
async fn records_reading_completions_and_returns_the_persisted_results() {
    let app = test_app().await;
    for (title, author) in [
        ("Programming Rust", "Jim Blandy"),
        ("Rust for Rustaceans", "Jon Gjengset"),
    ] {
        let response = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/books",
                json!({"title": title, "author": author}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }
    for id in [1, 2] {
        let response = app
            .clone()
            .oneshot(json_request(
                "PATCH",
                &format!("/books/{id}/status"),
                json!({"status": "reading"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/reading-completions",
            json!({
                "items": [
                    {"book_id": 2, "body": "Future が保持する値を確認した"},
                    {"book_id": 1, "body": "所有権の章を実装と結び付けて読めた"}
                ]
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await,
        json!({
            "results": [
                {
                    "book_id": 2,
                    "outcome": "completed",
                    "book": {
                        "id": 2,
                        "title": "Rust for Rustaceans",
                        "author": "Jon Gjengset",
                        "status": "finished"
                    },
                    "note": {"id": 1, "body": "Future が保持する値を確認した"}
                },
                {
                    "book_id": 1,
                    "outcome": "completed",
                    "book": {
                        "id": 1,
                        "title": "Programming Rust",
                        "author": "Jim Blandy",
                        "status": "finished"
                    },
                    "note": {"id": 2, "body": "所有権の章を実装と結び付けて読めた"}
                }
            ]
        })
    );

    for (book_id, note_id, expected_note) in [
        (1, 2, "所有権の章を実装と結び付けて読めた"),
        (2, 1, "Future が保持する値を確認した"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/books/{book_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let detail = json_body(response).await;
        assert_eq!(detail["status"], "finished");
        assert_eq!(
            detail["notes"],
            json!([{"id": note_id, "body": expected_note}])
        );
    }
}

#[tokio::test]
async fn rejects_the_entire_completion_request_before_saving_any_item() {
    let app = test_app().await;
    for id in 1..=2 {
        let response = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/books",
                json!({"title": format!("Rust {id}"), "author": "Author"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let response = app
            .clone()
            .oneshot(json_request(
                "PATCH",
                &format!("/books/{id}/status"),
                json!({"status": "reading"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/reading-completions",
            json!({
                "items": [
                    {"book_id": 1, "body": "保存されてはいけないメモ"},
                    {"book_id": 2, "body": "  "}
                ]
            }),
        ))
        .await
        .unwrap();

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

    for id in 1..=2 {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/books/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let detail = json_body(response).await;
        assert_eq!(detail["status"], "reading");
        assert_eq!(detail["notes"], json!([]));
    }
}

#[tokio::test]
async fn rejects_completion_requests_above_the_concurrency_limit() {
    let app = test_app().await;
    let response = app
        .oneshot(json_request(
            "POST",
            "/reading-completions",
            json!({
                "items": (1..=9)
                    .map(|book_id| json!({
                        "book_id": book_id,
                        "body": format!("note {book_id}")
                    }))
                    .collect::<Vec<_>>()
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(response).await,
        json!({
            "error": {
                "code": "validation_error",
                "message": "items must contain between 1 and 8 entries"
            }
        })
    );
}

#[tokio::test]
async fn returns_item_failures_without_rolling_back_independent_successes() {
    let app = test_app().await;
    for (title, status) in [
        ("Successful completion", Some("reading")),
        ("Conflicting completion", None),
    ] {
        let response = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/books",
                json!({"title": title, "author": "Author"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        if let Some(status) = status {
            let response = app
                .clone()
                .oneshot(json_request(
                    "PATCH",
                    "/books/1/status",
                    json!({"status": status}),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/reading-completions",
            json!({
                "items": [
                    {"book_id": 999, "body": "存在しない本"},
                    {"book_id": 2, "body": "読書中ではない本"},
                    {"book_id": 1, "body": "独立して成功する本"}
                ]
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await,
        json!({
            "results": [
                {
                    "book_id": 999,
                    "outcome": "failed",
                    "error": {
                        "code": "not_found",
                        "message": "resource not found"
                    }
                },
                {
                    "book_id": 2,
                    "outcome": "failed",
                    "error": {
                        "code": "conflict",
                        "message": "reading state conflict"
                    }
                },
                {
                    "book_id": 1,
                    "outcome": "completed",
                    "book": {
                        "id": 1,
                        "title": "Successful completion",
                        "author": "Author",
                        "status": "finished"
                    },
                    "note": {"id": 1, "body": "独立して成功する本"}
                }
            ]
        })
    );

    for (book_id, status, notes) in [
        (
            1,
            "finished",
            json!([{"id": 1, "body": "独立して成功する本"}]),
        ),
        (2, "want_to_read", json!([])),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/books/{book_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let detail = json_body(response).await;
        assert_eq!(detail["status"], status);
        assert_eq!(detail["notes"], notes);
    }
}

#[tokio::test]
async fn rolls_back_a_completion_when_adding_its_note_fails() {
    let (app, pool) = test_app_and_pool().await;
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/books",
            json!({"title": "Rollback", "author": "Author"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let response = app
        .clone()
        .oneshot(json_request(
            "PATCH",
            "/books/1/status",
            json!({"status": "reading"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    sqlx::query(
        "CREATE TRIGGER fail_completion_note
         BEFORE INSERT ON notes
         BEGIN
             SELECT RAISE(ABORT, 'injected note failure');
         END",
    )
    .execute(&pool)
    .await
    .unwrap();

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/reading-completions",
            json!({"items": [{"book_id": 1, "body": "保存されないメモ"}]}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json_body(response).await,
        json!({
            "results": [{
                "book_id": 1,
                "outcome": "failed",
                "error": {
                    "code": "internal_error",
                    "message": "internal server error"
                }
            }]
        })
    );

    let response = app
        .oneshot(Request::get("/books/1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let detail = json_body(response).await;
    assert_eq!(detail["status"], "reading");
    assert_eq!(detail["notes"], json!([]));
}
