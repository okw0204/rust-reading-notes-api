//! Axum の extractor と、アプリケーションの型との境界です。

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app::AppState,
    domain::{Book, BookId, ReadingStatus},
    error::AppError,
    service::{self, CreateBook},
};

#[derive(Deserialize)]
pub(crate) struct CreateBookRequest {
    title: String,
    author: String,
}

#[derive(Deserialize)]
pub(crate) struct ListBooksQuery {
    status: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct BookResponse {
    id: BookId,
    title: String,
    author: String,
    status: ReadingStatus,
}

impl From<Book> for BookResponse {
    fn from(book: Book) -> Self {
        let _created_at = book.created_at;
        Self {
            id: book.id,
            title: book.title,
            author: book.author,
            status: book.status,
        }
    }
}

pub(crate) async fn create_book(
    State(state): State<AppState>,
    Json(request): Json<CreateBookRequest>,
) -> Result<(StatusCode, Json<BookResponse>), AppError> {
    // handler は HTTP の値をユースケースの入力へ変え、検証規則は service に委ねる。
    let book = service::create_book(
        &state.pool,
        CreateBook {
            title: request.title,
            author: request.author,
        },
    )
    .await?;

    Ok((StatusCode::CREATED, Json(book.into())))
}

pub(crate) async fn list_books(
    State(state): State<AppState>,
    Query(query): Query<ListBooksQuery>,
) -> Result<Json<Vec<BookResponse>>, AppError> {
    let books = service::list_books(&state.pool, query.status).await?;
    Ok(Json(books.into_iter().map(BookResponse::from).collect()))
}
