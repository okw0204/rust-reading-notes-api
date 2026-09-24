//! Axum の extractor と、アプリケーションの型との境界です。

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::{
    app::AppState,
    domain::{BookDetail, BookId, Note, NoteId, ReadingStatus, StoredBook},
    error::AppError,
    repository::BookRepository,
    service::{
        AddNote, CompletedReading, CreateBook, RecordReadingCompletion, RecordReadingCompletions,
        UpdateStatus,
    },
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

// ANCHOR: reading_completions_request
#[derive(Deserialize)]
pub(crate) struct ReadingCompletionsRequest {
    items: Vec<ReadingCompletionItemRequest>,
}

#[derive(Deserialize)]
struct ReadingCompletionItemRequest {
    book_id: BookId,
    body: String,
}
// ANCHOR_END: reading_completions_request

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

#[derive(Serialize)]
pub(crate) struct ReadingCompletionsResponse {
    results: Vec<ReadingCompletionResultResponse>,
}

#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum ReadingCompletionResultResponse {
    Completed {
        book_id: BookId,
        book: BookResponse,
        note: NoteResponse,
    },
}

impl From<StoredBook> for BookResponse {
    fn from(book: StoredBook) -> Self {
        // DB 由来のドメイン型から、HTTP で公開する DTO へ所有権ごと移す。
        let (id, title, author, status) = book.into_parts();
        Self {
            id,
            title: title.into_inner(),
            author: author.into_inner(),
            status,
        }
    }
}

impl From<Note> for NoteResponse {
    fn from(note: Note) -> Self {
        let Note { id, body } = note;
        Self {
            id,
            body: body.into_inner(),
        }
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

impl From<CompletedReading> for ReadingCompletionResultResponse {
    fn from(completion: CompletedReading) -> Self {
        Self::Completed {
            book_id: completion.book.id(),
            book: completion.book.into(),
            note: completion.note.into(),
        }
    }
}

// ANCHOR: create_book_handler
pub(crate) async fn create_book<R: BookRepository>(
    State(state): State<AppState<R>>,
    Json(request): Json<CreateBookRequest>,
) -> Result<(StatusCode, Json<BookResponse>), AppError> {
    // handler は HTTP の値をユースケースの入力へ変え、検証規則は service に委ねる。
    let book = state
        .service
        .create_book(CreateBook {
            title: request.title,
            author: request.author,
        })
        .await?;

    Ok((StatusCode::CREATED, Json(book.into())))
}

// ANCHOR_END: create_book_handler

pub(crate) async fn list_books<R: BookRepository>(
    State(state): State<AppState<R>>,
    Query(query): Query<ListBooksQuery>,
) -> Result<Json<Vec<BookResponse>>, AppError> {
    let books = state.service.list_books(query.status).await?;
    Ok(Json(books.into_iter().map(BookResponse::from).collect()))
}

pub(crate) async fn get_book<R: BookRepository>(
    State(state): State<AppState<R>>,
    Path(id): Path<BookId>,
) -> Result<Json<BookDetailResponse>, AppError> {
    let detail = state.service.get_book(id).await?;
    Ok(Json(detail.into()))
}

pub(crate) async fn add_note<R: BookRepository>(
    State(state): State<AppState<R>>,
    Path(book_id): Path<BookId>,
    Json(request): Json<AddNoteRequest>,
) -> Result<(StatusCode, Json<NoteResponse>), AppError> {
    let note = state
        .service
        .add_note(book_id, AddNote { body: request.body })
        .await?;
    Ok((StatusCode::CREATED, Json(note.into())))
}

// ANCHOR: update_status_handler
pub(crate) async fn update_status<R: BookRepository>(
    State(state): State<AppState<R>>,
    Path(book_id): Path<BookId>,
    Json(request): Json<UpdateStatusRequest>,
) -> Result<Json<BookResponse>, AppError> {
    let book = state
        .service
        .update_status(
            book_id,
            UpdateStatus {
                status: request.status,
            },
        )
        .await?;
    Ok(Json(book.into()))
}
// ANCHOR_END: update_status_handler

// ANCHOR: reading_completions_handler
pub(crate) async fn record_reading_completions<R: BookRepository>(
    State(state): State<AppState<R>>,
    Json(request): Json<ReadingCompletionsRequest>,
) -> Result<Json<ReadingCompletionsResponse>, AppError> {
    let completions = state
        .service
        .record_reading_completions(RecordReadingCompletions {
            items: request
                .items
                .into_iter()
                .map(|item| RecordReadingCompletion {
                    book_id: item.book_id,
                    body: item.body,
                })
                .collect(),
        })
        .await?;

    Ok(Json(ReadingCompletionsResponse {
        results: completions
            .into_iter()
            .map(ReadingCompletionResultResponse::from)
            .collect(),
    }))
}
// ANCHOR_END: reading_completions_handler

pub(crate) async fn delete_book<R: BookRepository>(
    State(state): State<AppState<R>>,
    Path(book_id): Path<BookId>,
) -> Result<StatusCode, AppError> {
    state.service.delete_book(book_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
