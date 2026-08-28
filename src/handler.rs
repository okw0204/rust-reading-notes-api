//! Axum の extractor と、アプリケーションの型との境界です。

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app::AppState,
    domain::{Book, BookDetail, BookId, Note, NoteId, ReadingStatus},
    error::AppError,
    service::{self, AddNote, CreateBook, UpdateStatus},
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

#[derive(Deserialize)]
pub(crate) struct AddNoteRequest {
    body: String,
}

#[derive(Deserialize)]
pub(crate) struct UpdateStatusRequest {
    status: String,
}

#[derive(Serialize)]
pub(crate) struct BookResponse {
    id: BookId,
    title: String,
    author: String,
    status: ReadingStatus,
}

#[derive(Serialize)]
pub(crate) struct NoteResponse {
    id: NoteId,
    body: String,
}

#[derive(Serialize)]
pub(crate) struct BookDetailResponse {
    #[serde(flatten)]
    book: BookResponse,
    notes: Vec<NoteResponse>,
}

impl From<Book> for BookResponse {
    fn from(book: Book) -> Self {
        // DB 由来のドメイン型から、HTTP で公開する DTO へ所有権ごと移す。
        let Book {
            id,
            title,
            author,
            status,
        } = book;
        Self {
            id,
            title,
            author,
            status,
        }
    }
}

impl From<Note> for NoteResponse {
    fn from(note: Note) -> Self {
        let Note { id, body } = note;
        Self { id, body }
    }
}

impl From<BookDetail> for BookDetailResponse {
    fn from(detail: BookDetail) -> Self {
        Self {
            book: detail.book.into(),
            notes: detail.notes.into_iter().map(NoteResponse::from).collect(),
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

pub(crate) async fn get_book(
    State(state): State<AppState>,
    Path(id): Path<BookId>,
) -> Result<Json<BookDetailResponse>, AppError> {
    let detail = service::get_book(&state.pool, id).await?;
    Ok(Json(detail.into()))
}

pub(crate) async fn add_note(
    State(state): State<AppState>,
    Path(book_id): Path<BookId>,
    Json(request): Json<AddNoteRequest>,
) -> Result<(StatusCode, Json<NoteResponse>), AppError> {
    let note = service::add_note(&state.pool, book_id, AddNote { body: request.body }).await?;
    Ok((StatusCode::CREATED, Json(note.into())))
}

pub(crate) async fn update_status(
    State(state): State<AppState>,
    Path(book_id): Path<BookId>,
    Json(request): Json<UpdateStatusRequest>,
) -> Result<Json<BookResponse>, AppError> {
    let book = service::update_status(
        &state.pool,
        book_id,
        UpdateStatus {
            status: request.status,
        },
    )
    .await?;
    Ok(Json(book.into()))
}

pub(crate) async fn delete_book(
    State(state): State<AppState>,
    Path(book_id): Path<BookId>,
) -> Result<StatusCode, AppError> {
    service::delete_book(&state.pool, book_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
