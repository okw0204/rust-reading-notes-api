//! SQL とドメイン型の変換を担当するデータアクセス層です。

use sqlx::{FromRow, SqlitePool};

use crate::{
    domain::{Book, BookId, Note, NoteId, ReadingStatus},
    error::AppError,
};

#[derive(FromRow)]
// SQLx が復元する DB 表現を、公開したいドメイン型から分離する。
struct BookRow {
    id: i64,
    title: String,
    author: String,
    status: String,
}

#[derive(FromRow)]
struct NoteRow {
    id: i64,
    body: String,
}

impl TryFrom<BookRow> for Book {
    type Error = AppError;

    fn try_from(row: BookRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: BookId(row.id),
            title: row.title,
            author: row.author,
            status: ReadingStatus::try_from(row.status.as_str())?,
        })
    }
}

impl From<NoteRow> for Note {
    fn from(row: NoteRow) -> Self {
        Self {
            id: NoteId(row.id),
            body: row.body,
        }
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
        RETURNING id, title, author, status
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
    // Option の有無で SQL を分け、フィルターなしの意味を SQL 側でも明示する。
    let rows = match status {
        Some(status) => {
            sqlx::query_as::<_, BookRow>(
                "SELECT id, title, author, status FROM books WHERE status = ? ORDER BY id",
            )
            .bind(status.as_str())
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, BookRow>("SELECT id, title, author, status FROM books ORDER BY id")
                .fetch_all(pool)
                .await?
        }
    };

    rows.into_iter().map(Book::try_from).collect()
}

pub(crate) async fn find_book(pool: &SqlitePool, id: BookId) -> Result<Book, AppError> {
    let row =
        sqlx::query_as::<_, BookRow>("SELECT id, title, author, status FROM books WHERE id = ?")
            .bind(id.0)
            .fetch_optional(pool)
            .await?
            .ok_or(AppError::NotFound)?;

    row.try_into()
}

pub(crate) async fn list_notes(pool: &SqlitePool, book_id: BookId) -> Result<Vec<Note>, AppError> {
    let rows =
        sqlx::query_as::<_, NoteRow>("SELECT id, body FROM notes WHERE book_id = ? ORDER BY id")
            .bind(book_id.0)
            .fetch_all(pool)
            .await?;

    Ok(rows.into_iter().map(Note::from).collect())
}

pub(crate) async fn insert_note(
    pool: &SqlitePool,
    book_id: BookId,
    body: &str,
) -> Result<Note, AppError> {
    let row = sqlx::query_as::<_, NoteRow>(
        r#"
        INSERT INTO notes (book_id, body)
        VALUES (?, ?)
        RETURNING id, body
        "#,
    )
    .bind(book_id.0)
    .bind(body)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}

pub(crate) async fn update_book_status(
    pool: &SqlitePool,
    id: BookId,
    status: ReadingStatus,
) -> Result<Book, AppError> {
    let row = sqlx::query_as::<_, BookRow>(
        r#"
        UPDATE books
        SET status = ?
        WHERE id = ?
        RETURNING id, title, author, status
        "#,
    )
    .bind(status.as_str())
    .bind(id.0)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    row.try_into()
}

pub(crate) async fn delete_book(pool: &SqlitePool, id: BookId) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM books WHERE id = ?")
        .bind(id.0)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(())
}
