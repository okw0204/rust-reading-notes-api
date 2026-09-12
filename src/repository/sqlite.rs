//! SQL とドメイン型の変換を担当するデータアクセス層です。

use super::BookRepository;
use sqlx::{FromRow, SqlitePool};

use crate::{
    domain::{Author, BookId, BookTitle, Note, NoteBody, NoteId, ReadingStatus, StoredBook},
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

// ANCHOR: book_row_conversion
impl TryFrom<BookRow> for StoredBook {
    type Error = AppError;

    fn try_from(row: BookRow) -> Result<Self, Self::Error> {
        Ok(Self::restore(
            BookId(row.id),
            // 保存済みの値の不正は、リクエストの検証失敗と区別する。
            BookTitle::try_from(row.title)
                .map_err(|error| AppError::InvalidStoredValue(error.to_string()))?,
            Author::try_from(row.author)
                .map_err(|error| AppError::InvalidStoredValue(error.to_string()))?,
            ReadingStatus::try_from(row.status.as_str())?,
        ))
    }
}

// ANCHOR_END: book_row_conversion

impl TryFrom<NoteRow> for Note {
    type Error = AppError;

    fn try_from(row: NoteRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: NoteId(row.id),
            body: NoteBody::try_from(row.body)
                .map_err(|error| AppError::InvalidStoredValue(error.to_string()))?,
        })
    }
}

pub(crate) struct SqliteBookRepository {
    pool: SqlitePool,
}

impl SqliteBookRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl BookRepository for SqliteBookRepository {
    // ANCHOR: insert_book_sqlite
    async fn insert_book(
        &self,
        title: &BookTitle,
        author: &Author,
    ) -> Result<StoredBook, AppError> {
        let row = sqlx::query_as::<_, BookRow>(
            r#"
        INSERT INTO books (title, author, status)
        VALUES (?, ?, 'want_to_read')
        RETURNING id, title, author, status
        "#,
        )
        .bind(title.as_str())
        .bind(author.as_str())
        .fetch_one(&self.pool)
        .await?;

        row.try_into()
    }

    // ANCHOR_END: insert_book_sqlite

    async fn list_books(&self, status: Option<ReadingStatus>) -> Result<Vec<StoredBook>, AppError> {
        // Option の有無で SQL を分け、フィルターなしの意味を SQL 側でも明示する。
        let rows = match status {
            Some(status) => {
                sqlx::query_as::<_, BookRow>(
                    "SELECT id, title, author, status FROM books WHERE status = ? ORDER BY id",
                )
                .bind(status.as_str())
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, BookRow>(
                    "SELECT id, title, author, status FROM books ORDER BY id",
                )
                .fetch_all(&self.pool)
                .await?
            }
        };

        rows.into_iter().map(StoredBook::try_from).collect()
    }

    // ANCHOR: find_book_sqlite
    async fn find_book(&self, id: BookId) -> Result<StoredBook, AppError> {
        let row = sqlx::query_as::<_, BookRow>(
            "SELECT id, title, author, status FROM books WHERE id = ?",
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AppError::NotFound)?;

        row.try_into()
    }

    // ANCHOR_END: find_book_sqlite

    async fn list_notes(&self, book_id: BookId) -> Result<Vec<Note>, AppError> {
        let rows = sqlx::query_as::<_, NoteRow>(
            "SELECT id, body FROM notes WHERE book_id = ? ORDER BY id",
        )
        .bind(book_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(Note::try_from).collect()
    }

    // ANCHOR: atomic_note_insert
    async fn insert_note(&self, book_id: BookId, body: &NoteBody) -> Result<Note, AppError> {
        let row = sqlx::query_as::<_, NoteRow>(
            r#"
        INSERT INTO notes (book_id, body)
        SELECT ?, ?
        WHERE EXISTS (SELECT 1 FROM books WHERE id = ?)
        RETURNING id, body
        "#,
        )
        .bind(book_id.0)
        .bind(body.as_str())
        .bind(book_id.0)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AppError::NotFound)?;

        row.try_into()
    }

    // ANCHOR_END: atomic_note_insert

    // ANCHOR: conditional_status_sqlite
    // ANCHOR: conditional_update
    async fn update_book_status(
        &self,
        expected: ReadingStatus,
        next: StoredBook,
    ) -> Result<StoredBook, AppError> {
        let row = sqlx::query_as::<_, BookRow>(
            r#"
        UPDATE books
        SET status = ?
        WHERE id = ? AND status = ?
        RETURNING id, title, author, status
        "#,
        )
        .bind(next.status().as_str())
        .bind(next.id().0)
        .bind(expected.as_str())
        .fetch_optional(&self.pool)
        .await?
        // 取得後の状態変更も削除も、期待した状態で保存できなかった競合として扱う。
        .ok_or(AppError::Conflict)?;

        row.try_into()
    }

    // ANCHOR_END: conditional_update
    // ANCHOR_END: conditional_status_sqlite

    async fn delete_book(&self, id: BookId) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM books WHERE id = ?")
            .bind(id.0)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ANCHOR: stale_update_test
    #[sqlx::test(migrations = "./migrations")]
    async fn conditional_update_rejects_a_stale_state(pool: SqlitePool) {
        let repository = SqliteBookRepository::new(pool);
        let title = BookTitle::try_from("Book".to_owned()).unwrap();
        let author = Author::try_from("Author".to_owned()).unwrap();
        let current = repository.insert_book(&title, &author).await.unwrap();
        let first = repository.find_book(current.id()).await.unwrap();
        let second = repository.find_book(current.id()).await.unwrap();
        let StoredBook::WantToRead(first) = first else {
            panic!("取得した書籍は未読")
        };
        let StoredBook::WantToRead(second) = second else {
            panic!("二度目に取得した書籍も未読")
        };
        let saved = repository
            .update_book_status(
                ReadingStatus::WantToRead,
                StoredBook::Reading(first.start_reading()),
            )
            .await
            .unwrap();
        assert_eq!(saved.status(), ReadingStatus::Reading);
        // 別の更新が完了したあとに、古い取得結果を使って更新する。
        let error = repository
            .update_book_status(
                ReadingStatus::WantToRead,
                StoredBook::Reading(second.start_reading()),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Conflict));
        assert_eq!(
            repository.find_book(saved.id()).await.unwrap().status(),
            ReadingStatus::Reading
        );
    }

    // ANCHOR_END: stale_update_test

    #[sqlx::test(migrations = "./migrations")]
    async fn conditional_update_rejects_deletion_after_fetch(pool: SqlitePool) {
        let repository = SqliteBookRepository::new(pool);
        let title = BookTitle::try_from("Book".to_owned()).unwrap();
        let author = Author::try_from("Author".to_owned()).unwrap();
        let current = repository.insert_book(&title, &author).await.unwrap();
        let fetched = repository.find_book(current.id()).await.unwrap();
        let StoredBook::WantToRead(book) = fetched else {
            panic!("取得した書籍は未読")
        };
        let id = book.id();
        let next = StoredBook::Reading(book.start_reading());
        repository.delete_book(id).await.unwrap();
        let error = repository
            .update_book_status(ReadingStatus::WantToRead, next)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Conflict));
        assert!(matches!(
            repository.find_book(id).await,
            Err(AppError::NotFound)
        ));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn find_book_rejects_invalid_stored_text(pool: SqlitePool) {
        let repository = SqliteBookRepository::new(pool.clone());
        for (title, author, message) in [
            (" \t", "Author", "title must not be empty"),
            ("Title", " \n", "author must not be empty"),
        ] {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO books (title, author, status) VALUES (?, ?, 'want_to_read') RETURNING id",
            )
            .bind(title)
            .bind(author)
            .fetch_one(&pool)
            .await
            .unwrap();
            let error = repository.find_book(BookId(id)).await.unwrap_err();
            assert!(matches!(error, AppError::InvalidStoredValue(ref value) if value == message));
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn list_notes_rejects_invalid_stored_body(pool: SqlitePool) {
        let repository = SqliteBookRepository::new(pool.clone());
        let title = BookTitle::try_from("Title".to_owned()).unwrap();
        let author = Author::try_from("Author".to_owned()).unwrap();
        let book = repository.insert_book(&title, &author).await.unwrap();
        sqlx::query("INSERT INTO notes (book_id, body) VALUES (?, ?)")
            .bind(book.id().0)
            .bind(" \t\n")
            .execute(&pool)
            .await
            .unwrap();
        let error = repository.list_notes(book.id()).await.unwrap_err();
        assert!(
            matches!(error, AppError::InvalidStoredValue(ref value) if value == "note body must not be empty")
        );
    }

    #[test]
    fn rejects_an_unknown_stored_status() {
        // 通常は SQLite の CHECK が拒否する値を、復元境界でも拒否する。
        let error = StoredBook::try_from(BookRow {
            id: 1,
            title: "Title".into(),
            author: "Author".into(),
            status: "paused".into(),
        })
        .unwrap_err();
        assert!(matches!(error, AppError::InvalidStoredValue(_)));
    }

    #[test]
    fn stored_book_text_is_validated_and_trimmed() {
        for (title, author, message) in [
            (" \t", "Author", "title must not be empty"),
            ("Book", "\n", "author must not be empty"),
        ] {
            let error = StoredBook::try_from(BookRow {
                id: 1,
                title: title.to_owned(),
                author: author.to_owned(),
                status: "want_to_read".to_owned(),
            })
            .unwrap_err();
            assert!(matches!(error, AppError::InvalidStoredValue(ref value) if value == message));
        }
        let book = StoredBook::try_from(BookRow {
            id: 1,
            title: " Book \n".to_owned(),
            author: " Author \t".to_owned(),
            status: "reading".to_owned(),
        })
        .unwrap();
        let (_, title, author, status) = book.into_parts();
        assert_eq!(title.as_str(), "Book");
        assert_eq!(author.as_str(), "Author");
        assert_eq!(status, ReadingStatus::Reading);
    }

    #[test]
    fn stored_note_text_is_validated_and_trimmed() {
        let error = Note::try_from(NoteRow {
            id: 1,
            body: " \n".to_owned(),
        })
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid value stored in database: note body must not be empty"
        );
        let note = Note::try_from(NoteRow {
            id: 1,
            body: " メモ\n本文 \t".to_owned(),
        })
        .unwrap();
        assert_eq!(note.body.as_str(), "メモ\n本文");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn inserting_a_note_for_an_unknown_book_returns_not_found(pool: SqlitePool) {
        let repository = SqliteBookRepository::new(pool);
        let body = NoteBody::try_from("note".to_owned()).unwrap();
        let error = repository
            .insert_note(BookId(999), &body)
            .await
            .unwrap_err();

        assert!(matches!(error, AppError::NotFound));
    }
}
