//! HTTP の詳細を知らずに、入力規則とユースケースを実行します。

use std::collections::HashSet;

use crate::{
    domain::{Author, BookDetail, BookId, BookTitle, Note, NoteBody, ReadingStatus, StoredBook},
    error::AppError,
    repository::BookRepository,
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

pub(crate) struct RecordReadingCompletion {
    pub(crate) book_id: BookId,
    pub(crate) body: String,
}

pub(crate) struct RecordReadingCompletions {
    pub(crate) items: Vec<RecordReadingCompletion>,
}

pub(crate) struct CompletedReading {
    pub(crate) book: StoredBook,
    pub(crate) note: Note,
}

// ANCHOR: generic_service
pub(crate) struct ReadingService<R> {
    repository: R,
}

impl<R: BookRepository> ReadingService<R> {
    pub(crate) fn new(repository: R) -> Self {
        Self { repository }
    }
    // ANCHOR_END: generic_service

    // ANCHOR: create_book_service
    pub(crate) async fn create_book(&self, input: CreateBook) -> Result<StoredBook, AppError> {
        let title = BookTitle::try_from(input.title)?;
        let author = Author::try_from(input.author)?;
        self.repository.insert_book(&title, &author).await
    }

    // ANCHOR_END: create_book_service

    pub(crate) async fn list_books(
        &self,
        status: Option<String>,
    ) -> Result<Vec<StoredBook>, AppError> {
        // Option<Result<T, E>> を Result<Option<T>, E> に裏返すと、? で検証失敗を返せる。
        let status = status
            .as_deref()
            .map(ReadingStatus::parse_input)
            .transpose()?;
        self.repository.list_books(status).await
    }

    // ANCHOR: get_book_service
    pub(crate) async fn get_book(&self, id: BookId) -> Result<BookDetail, AppError> {
        let book = self.repository.find_book(id).await?;
        let notes = self.repository.list_notes(id).await?;
        Ok(BookDetail { book, notes })
    }

    // ANCHOR_END: get_book_service

    pub(crate) async fn add_note(&self, book_id: BookId, input: AddNote) -> Result<Note, AppError> {
        let body = NoteBody::try_from(input.body)?;

        // 存在確認と追加の間に削除が割り込む場合も、repository が NotFound にする。
        self.repository.insert_note(book_id, &body).await
    }

    // ANCHOR: update_status_service
    pub(crate) async fn update_status(
        &self,
        book_id: BookId,
        input: UpdateStatus,
    ) -> Result<StoredBook, AppError> {
        let requested = ReadingStatus::parse_input(&input.status)?;
        let current = self.repository.find_book(book_id).await?;
        let expected = current.status();
        let next = match (current, requested) {
            (StoredBook::WantToRead(book), ReadingStatus::Reading) => {
                StoredBook::Reading(book.start_reading())
            }
            (StoredBook::Reading(book), ReadingStatus::Finished) => {
                StoredBook::Finished(book.finish())
            }
            _ => return Err(AppError::Conflict),
        };
        self.repository.update_book_status(expected, next).await
    }

    // ANCHOR_END: update_status_service

    // ANCHOR: reading_completions_service
    pub(crate) async fn record_reading_completions(
        &self,
        input: RecordReadingCompletions,
    ) -> Result<Vec<CompletedReading>, AppError> {
        // ANCHOR: reading_completions_validation
        if !(1..=8).contains(&input.items.len()) {
            return Err(AppError::Validation(
                "items must contain between 1 and 8 entries".to_owned(),
            ));
        }

        let mut seen_book_ids = HashSet::with_capacity(input.items.len());
        let mut validated = Vec::with_capacity(input.items.len());
        for item in input.items {
            if !seen_book_ids.insert(item.book_id.0) {
                return Err(AppError::Validation(
                    "book_id must not be duplicated".to_owned(),
                ));
            }
            validated.push((item.book_id, NoteBody::try_from(item.body)?));
        }
        // ANCHOR_END: reading_completions_validation

        let mut completions = Vec::with_capacity(validated.len());
        for (book_id, body) in validated {
            let current = self.repository.find_book(book_id).await?;
            let reading = match current {
                StoredBook::Reading(book) => book,
                _ => return Err(AppError::Conflict),
            };
            let (book, note) = self
                .repository
                .record_reading_completion(reading.finish(), &body)
                .await?;
            completions.push(CompletedReading { book, note });
        }
        Ok(completions)
    }
    // ANCHOR_END: reading_completions_service

    pub(crate) async fn delete_book(&self, book_id: BookId) -> Result<(), AppError> {
        self.repository.delete_book(book_id).await
    }
}

#[cfg(test)]
mod tests;
