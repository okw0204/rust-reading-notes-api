//! HTTP の詳細を知らずに、入力規則とユースケースを実行します。

use sqlx::SqlitePool;

use crate::{
    domain::{Book, BookDetail, BookId, Note, ReadingStatus},
    error::AppError,
    repository,
};

pub(crate) struct CreateBook {
    pub(crate) title: String,
    pub(crate) author: String,
}

pub(crate) struct AddNote {
    pub(crate) body: String,
}

pub(crate) struct UpdateStatus {
    pub(crate) status: String,
}

pub(crate) async fn create_book(
    pool: &SqlitePool,
    input: CreateBook,
) -> Result<Book, AppError> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("title must not be empty".to_owned()));
    }

    let author = input.author.trim();
    if author.is_empty() {
        return Err(AppError::Validation("author must not be empty".to_owned()));
    }

    repository::insert_book(pool, title, author).await
}

pub(crate) async fn list_books(
    pool: &SqlitePool,
    status: Option<String>,
) -> Result<Vec<Book>, AppError> {
    let status = status
        .as_deref()
        .map(crate::domain::ReadingStatus::parse_filter)
        .transpose()?;
    repository::list_books(pool, status).await
}

pub(crate) async fn get_book(pool: &SqlitePool, id: BookId) -> Result<BookDetail, AppError> {
    let book = repository::find_book(pool, id).await?;
    let notes = repository::list_notes(pool, id).await?;
    Ok(BookDetail { book, notes })
}

pub(crate) async fn add_note(
    pool: &SqlitePool,
    book_id: BookId,
    input: AddNote,
) -> Result<Note, AppError> {
    let body = input.body.trim();
    if body.is_empty() {
        return Err(AppError::Validation(
            "note body must not be empty".to_owned(),
        ));
    }

    // 外部キー違反を 500 にせず、利用者が理解できる 404 に変えるため先に存在確認する。
    repository::find_book(pool, book_id).await?;
    repository::insert_note(pool, book_id, body).await
}

pub(crate) async fn update_status(
    pool: &SqlitePool,
    book_id: BookId,
    input: UpdateStatus,
) -> Result<Book, AppError> {
    let status = ReadingStatus::parse_filter(&input.status)?;
    repository::update_book_status(pool, book_id, status).await
}

pub(crate) async fn delete_book(pool: &SqlitePool, book_id: BookId) -> Result<(), AppError> {
    repository::delete_book(pool, book_id).await
}
