//! SQL とドメイン型の変換を担当するデータアクセス層です。

use chrono::NaiveDateTime;
use sqlx::{FromRow, SqlitePool};

use crate::{
    domain::{Book, BookId, ReadingStatus},
    error::AppError,
};

#[derive(FromRow)]
struct BookRow {
    id: i64,
    title: String,
    author: String,
    status: String,
    created_at: NaiveDateTime,
}

impl TryFrom<BookRow> for Book {
    type Error = AppError;

    fn try_from(row: BookRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: BookId(row.id),
            title: row.title,
            author: row.author,
            status: ReadingStatus::try_from(row.status.as_str())?,
            created_at: row.created_at,
        })
    }
}

pub(crate) async fn insert_book(
    pool: &SqlitePool,
    title: &str,
    author: &str,
) -> Result<Book, AppError> {
    let row = sqlx::query_as::<_, BookRow>(
        r#"
        INSERT INTO books (title, author, status)
        VALUES (?, ?, 'want_to_read')
        RETURNING id, title, author, status, created_at
        "#,
    )
    .bind(title)
    .bind(author)
    .fetch_one(pool)
    .await?;

    row.try_into()
}

pub(crate) async fn list_books(
    pool: &SqlitePool,
    status: Option<ReadingStatus>,
) -> Result<Vec<Book>, AppError> {
    let rows = match status {
        Some(status) => sqlx::query_as::<_, BookRow>(
            "SELECT id, title, author, status, created_at FROM books WHERE status = ? ORDER BY id",
        )
        .bind(status.as_str())
        .fetch_all(pool)
        .await?,
        None => sqlx::query_as::<_, BookRow>(
            "SELECT id, title, author, status, created_at FROM books ORDER BY id",
        )
        .fetch_all(pool)
        .await?,
    };

    rows.into_iter().map(Book::try_from).collect()
}
