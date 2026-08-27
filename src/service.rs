//! HTTP の詳細を知らずに、入力規則とユースケースを実行します。

use sqlx::SqlitePool;

use crate::{domain::Book, error::AppError, repository};

pub(crate) struct CreateBook {
    pub(crate) title: String,
    pub(crate) author: String,
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
