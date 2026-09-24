//! DB に接続せず、保存結果、失敗、並行処理の進行を制御するテスト用実装です。

use std::{collections::BTreeMap, sync::Arc};

use parking_lot::Mutex;
use tokio::sync::{Notify, Semaphore};

use crate::{
    domain::{
        Author, Book, BookId, BookTitle, Finished, Note, NoteBody, NoteId, ReadingStatus,
        StoredBook,
    },
    error::AppError,
    repository::BookRepository,
};

// ANCHOR: fake_state
#[derive(Clone, Default)]
pub(crate) struct FakeBookRepository {
    state: Arc<Mutex<FakeState>>,
}

#[derive(Clone)]
pub(crate) struct ReadingCompletionControl {
    inner: Arc<ReadingCompletionControlInner>,
}

struct ReadingCompletionControlInner {
    gates: BTreeMap<i64, Arc<Semaphore>>,
    progress: Mutex<ReadingCompletionProgress>,
    changed: Notify,
}

#[derive(Default)]
struct ReadingCompletionProgress {
    started: Vec<BookId>,
    finished: Vec<BookId>,
}

#[derive(Default)]
struct FakeState {
    books: BTreeMap<i64, StoredBook>,
    notes: BTreeMap<i64, Vec<Note>>,
    next_book_id: i64,
    next_note_id: i64,
    next_update_error: Option<AppError>,
    calls: usize,
    reading_completion_control: Option<ReadingCompletionControl>,
}

impl FakeBookRepository {
    pub(crate) fn fail_next_update(&self, error: AppError) {
        self.state.lock().next_update_error = Some(error);
    }

    pub(crate) fn calls(&self) -> usize {
        self.state.lock().calls
    }

    pub(crate) fn control_reading_completions(
        &self,
        book_ids: impl IntoIterator<Item = BookId>,
    ) -> ReadingCompletionControl {
        let control = ReadingCompletionControl {
            inner: Arc::new(ReadingCompletionControlInner {
                gates: book_ids
                    .into_iter()
                    .map(|book_id| (book_id.0, Arc::new(Semaphore::new(0))))
                    .collect(),
                progress: Mutex::new(ReadingCompletionProgress::default()),
                changed: Notify::new(),
            }),
        };
        self.state.lock().reading_completion_control = Some(control.clone());
        control
    }
}
impl ReadingCompletionControl {
    async fn wait_for_release(&self, book_id: BookId) {
        if let Some(gate) = self.inner.gates.get(&book_id.0) {
            gate.acquire()
                .await
                .expect("reading completion gate remains open")
                .forget();
        }
    }

    fn mark_started(&self, book_id: BookId) {
        self.inner.progress.lock().started.push(book_id);
        self.inner.changed.notify_waiters();
    }

    fn mark_finished(&self, book_id: BookId) {
        self.inner.progress.lock().finished.push(book_id);
        self.inner.changed.notify_waiters();
    }

    pub(crate) async fn wait_for_started(&self, count: usize) {
        loop {
            let changed = self.inner.changed.notified();
            if self.inner.progress.lock().started.len() >= count {
                return;
            }
            changed.await;
        }
    }

    pub(crate) async fn wait_for_finished(&self, count: usize) {
        loop {
            let changed = self.inner.changed.notified();
            if self.inner.progress.lock().finished.len() >= count {
                return;
            }
            changed.await;
        }
    }

    pub(crate) fn release(&self, book_id: BookId) {
        self.inner
            .gates
            .get(&book_id.0)
            .expect("controlled reading completion has a gate")
            .add_permits(1);
    }

    pub(crate) fn finished(&self) -> Vec<BookId> {
        self.inner.progress.lock().finished.clone()
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
        let mut state = self.state.lock();
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
        let mut state = self.state.lock();
        state.calls += 1;
        Ok(state
            .books
            .values()
            .filter(|book| status.is_none_or(|status| book.status() == status))
            .cloned()
            .collect())
    }

    async fn find_book(&self, id: BookId) -> Result<StoredBook, AppError> {
        let mut state = self.state.lock();
        state.calls += 1;
        state.books.get(&id.0).cloned().ok_or(AppError::NotFound)
    }

    async fn list_notes(&self, book_id: BookId) -> Result<Vec<Note>, AppError> {
        let mut state = self.state.lock();
        state.calls += 1;
        Ok(state.notes.get(&book_id.0).cloned().unwrap_or_default())
    }

    async fn insert_note(&self, book_id: BookId, body: &NoteBody) -> Result<Note, AppError> {
        let mut state = self.state.lock();
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
        let mut state = self.state.lock();
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

    async fn record_reading_completion(
        &self,
        book: Book<Finished>,
        body: &NoteBody,
    ) -> Result<(StoredBook, Note), AppError> {
        let book_id = book.id();
        let control = self.state.lock().reading_completion_control.clone();
        if let Some(control) = &control {
            control.mark_started(book_id);
            control.wait_for_release(book_id).await;
        }

        let mut state = self.state.lock();
        state.calls += 1;
        if let Some(error) = state.next_update_error.take() {
            return Err(error);
        }

        let finished = StoredBook::Finished(book);
        let id = finished.id().0;
        if state.books.get(&id).map(StoredBook::status) != Some(ReadingStatus::Reading) {
            return Err(AppError::Conflict);
        }

        state.next_note_id += 1;
        let note = Note {
            id: NoteId(state.next_note_id),
            body: body.clone(),
        };
        state.books.insert(id, finished.clone());
        state.notes.entry(id).or_default().push(note.clone());
        drop(state);

        if let Some(control) = control {
            control.mark_finished(book_id);
        }
        Ok((finished, note))
    }

    async fn delete_book(&self, id: BookId) -> Result<(), AppError> {
        let mut state = self.state.lock();
        state.calls += 1;
        state.books.remove(&id.0).ok_or(AppError::NotFound)?;
        state.notes.remove(&id.0);
        Ok(())
    }
}
