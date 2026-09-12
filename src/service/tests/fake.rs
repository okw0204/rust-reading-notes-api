//! DB に接続せず、保存結果と一度だけの保存失敗を制御するテスト用実装です。

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use crate::{
    domain::{Author, Book, BookId, BookTitle, Note, NoteBody, NoteId, ReadingStatus, StoredBook},
    error::AppError,
    repository::BookRepository,
};

// ANCHOR: fake_state
#[derive(Clone, Default)]
pub(super) struct FakeBookRepository {
    state: Arc<Mutex<FakeState>>,
}

#[derive(Default)]
struct FakeState {
    books: BTreeMap<i64, StoredBook>,
    notes: BTreeMap<i64, Vec<Note>>,
    next_book_id: i64,
    next_note_id: i64,
    next_update_error: Option<AppError>,
    calls: usize,
}

impl FakeBookRepository {
    pub(super) fn fail_next_update(&self, error: AppError) {
        self.state.lock().unwrap().next_update_error = Some(error);
    }

    pub(super) fn calls(&self) -> usize {
        self.state.lock().unwrap().calls
    }
}

// ANCHOR_END: fake_state

// ロック中は小さなメモリ操作だけを行い、await を挟まない。
impl BookRepository for FakeBookRepository {
    async fn insert_book(
        &self,
        title: &BookTitle,
        author: &Author,
    ) -> Result<StoredBook, AppError> {
        let mut state = self.state.lock().unwrap();
        state.calls += 1;
        state.next_book_id += 1;
        let book = StoredBook::WantToRead(Book::new(
            BookId(state.next_book_id),
            title.clone(),
            author.clone(),
        ));
        state.books.insert(book.id().0, book.clone());
        Ok(book)
    }

    async fn list_books(&self, status: Option<ReadingStatus>) -> Result<Vec<StoredBook>, AppError> {
        let mut state = self.state.lock().unwrap();
        state.calls += 1;
        Ok(state
            .books
            .values()
            .filter(|book| status.is_none_or(|status| book.status() == status))
            .cloned()
            .collect())
    }

    async fn find_book(&self, id: BookId) -> Result<StoredBook, AppError> {
        let mut state = self.state.lock().unwrap();
        state.calls += 1;
        state.books.get(&id.0).cloned().ok_or(AppError::NotFound)
    }

    async fn list_notes(&self, book_id: BookId) -> Result<Vec<Note>, AppError> {
        let mut state = self.state.lock().unwrap();
        state.calls += 1;
        Ok(state.notes.get(&book_id.0).cloned().unwrap_or_default())
    }

    async fn insert_note(&self, book_id: BookId, body: &NoteBody) -> Result<Note, AppError> {
        let mut state = self.state.lock().unwrap();
        state.calls += 1;
        if !state.books.contains_key(&book_id.0) {
            return Err(AppError::NotFound);
        }
        state.next_note_id += 1;
        let note = Note {
            id: NoteId(state.next_note_id),
            body: body.clone(),
        };
        state.notes.entry(book_id.0).or_default().push(note.clone());
        Ok(note)
    }

    // ANCHOR: fake_update
    async fn update_book_status(
        &self,
        expected: ReadingStatus,
        next: StoredBook,
    ) -> Result<StoredBook, AppError> {
        let mut state = self.state.lock().unwrap();
        state.calls += 1;
        if let Some(error) = state.next_update_error.take() {
            return Err(error);
        }
        let id = next.id().0;
        if state.books.get(&id).map(StoredBook::status) != Some(expected) {
            return Err(AppError::Conflict);
        }
        state.books.insert(id, next.clone());
        Ok(next)
    }

    // ANCHOR_END: fake_update

    async fn delete_book(&self, id: BookId) -> Result<(), AppError> {
        let mut state = self.state.lock().unwrap();
        state.calls += 1;
        state.books.remove(&id.0).ok_or(AppError::NotFound)?;
        state.notes.remove(&id.0);
        Ok(())
    }
}
