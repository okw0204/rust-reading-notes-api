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

pub(crate) async fn create_book(pool: &SqlitePool, input: CreateBook) -> Result<Book, AppError> {
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
    // Option<Result<T, E>> を Result<Option<T>, E> に裏返すと、? で検証失敗を返せる。
    let status = status
        .as_deref()
        .map(crate::domain::ReadingStatus::parse_input)
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

    // repository の単一 SQL が、存在確認と追加の間に削除が割り込む競合も 404 にする。
    repository::insert_note(pool, book_id, body).await
}

pub(crate) async fn update_status(
    pool: &SqlitePool,
    book_id: BookId,
    input: UpdateStatus,
) -> Result<Book, AppError> {
    let status = ReadingStatus::parse_input(&input.status)?;
    repository::update_book_status(pool, book_id, status).await
}

pub(crate) async fn delete_book(pool: &SqlitePool, book_id: BookId) -> Result<(), AppError> {
    repository::delete_book(pool, book_id).await
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    fn lazy_pool() -> SqlitePool {
        // 検証が DB より先に走ることも、接続しない pool によって確認できる。
        SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap()
    }

    #[tokio::test]
    async fn create_book_rejects_a_blank_title_before_accessing_the_database() {
        let error = create_book(
            &lazy_pool(),
            CreateBook {
                title: "  ".to_owned(),
                author: "Author".to_owned(),
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, AppError::Validation(_)));
    }

    #[tokio::test]
    async fn add_note_rejects_a_blank_body_before_accessing_the_database() {
        let error = add_note(
            &lazy_pool(),
            BookId(1),
            AddNote {
                body: "\t".to_owned(),
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, AppError::Validation(_)));
    }
}
