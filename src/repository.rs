//! service が利用する永続化の契約を定義します。

use std::future::Future;

use crate::{
    domain::{
        Author, Book, BookId, BookTitle, Finished, Note, NoteBody, ReadingStatus, StoredBook,
    },
    error::AppError,
};

mod sqlite;
pub(crate) use sqlite::SqliteBookRepository;

// ANCHOR: repository_contract
/// 実装と返される Future をスレッド間で扱える永続化境界です。
pub(crate) trait BookRepository: Send + Sync {
    /// 検証済みのタイトルと著者で未読の本を登録し、採番済みの本を返します。
    fn insert_book(
        &self,
        title: &BookTitle,
        author: &Author,
    ) -> impl Future<Output = Result<StoredBook, AppError>> + Send;

    /// 指定状態で絞った本を ID 順に返します。指定なしは全件、該当なしは空です。
    fn list_books(
        &self,
        status: Option<ReadingStatus>,
    ) -> impl Future<Output = Result<Vec<StoredBook>, AppError>> + Send;

    /// ID に対応する本を返し、未検出なら NotFound を返します。
    fn find_book(&self, id: BookId) -> impl Future<Output = Result<StoredBook, AppError>> + Send;

    /// 本のメモを ID 順に返します。本やメモがなければ空の一覧を返します。
    fn list_notes(
        &self,
        book_id: BookId,
    ) -> impl Future<Output = Result<Vec<Note>, AppError>> + Send;

    /// 検証済みの本文を追加します。対象の本がなければ NotFound を返します。
    fn insert_note(
        &self,
        book_id: BookId,
        body: &NoteBody,
    ) -> impl Future<Output = Result<Note, AppError>> + Send;

    /// 現在状態が expected と一致する場合だけ、遷移済みの next の状態を保存します。
    /// 状態の不一致や取得後の削除は Conflict とし、保存内容を変更しません。
    fn update_book_status(
        &self,
        expected: ReadingStatus,
        next: StoredBook,
    ) -> impl Future<Output = Result<StoredBook, AppError>> + Send;

    /// 読書中の本を読了へ進め、メモとともに一つの保存単位で確定します。
    /// 現在状態が読書中でなければ Conflict とし、状態とメモのどちらも変更しません。
    /// 依存先の失敗時も transaction を rollback し、片方だけを残しません。
    fn record_reading_completion(
        &self,
        book: Book<Finished>,
        body: &NoteBody,
    ) -> impl Future<Output = Result<(StoredBook, Note), AppError>> + Send;

    /// 本と対応するメモを削除します。本がなければ NotFound を返します。
    fn delete_book(&self, id: BookId) -> impl Future<Output = Result<(), AppError>> + Send;
}
// ANCHOR_END: repository_contract
