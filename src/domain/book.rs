use super::{Author, BookId, BookTitle, ReadingStatus};

// ANCHOR: book_typestate
#[derive(Debug, Clone)]
pub struct WantToRead;

#[derive(Debug, Clone)]
pub struct Reading;

#[derive(Debug, Clone)]
pub struct Finished;

/// 読書状態を型で表す本です。遷移は所有権を受け取り、次の状態の本を返します。
/// 新規作成は未読のみで、読書中を経て読了へ進めます。
///
/// ```rust
/// use rust_reading_notes_api::domain::{Author, Book, BookId, BookTitle};
/// let book = Book::new(
///     BookId(1),
///     BookTitle::try_from("Rust Book".to_owned()).unwrap(),
///     Author::try_from("Author".to_owned()).unwrap(),
/// );
/// let finished = book.start_reading().finish();
/// assert_eq!(finished.id(), BookId(1));
/// ```
///
/// 未読の本には `finish` がないため、読書中を飛び越せません。
///
/// ```compile_fail,E0599
/// use rust_reading_notes_api::domain::{Author, Book, BookId, BookTitle};
/// let book = Book::new(
///     BookId(1),
///     BookTitle::try_from("Rust Book".to_owned()).unwrap(),
///     Author::try_from("Author".to_owned()).unwrap(),
/// );
/// book.finish();
/// ```
#[derive(Debug, Clone)]
pub struct Book<S> {
    id: BookId,
    title: BookTitle,
    author: Author,
    state: S,
}

impl Book<WantToRead> {
    pub fn new(id: BookId, title: BookTitle, author: Author) -> Self {
        Self {
            id,
            title,
            author,
            state: WantToRead,
        }
    }

    pub fn start_reading(self) -> Book<Reading> {
        Book {
            id: self.id,
            title: self.title,
            author: self.author,
            state: Reading,
        }
    }
}

impl Book<Reading> {
    pub fn finish(self) -> Book<Finished> {
        Book {
            id: self.id,
            title: self.title,
            author: self.author,
            state: Finished,
        }
    }
}

impl<S> Book<S> {
    pub fn id(&self) -> BookId {
        self.id
    }
    pub fn title(&self) -> &BookTitle {
        &self.title
    }
    pub fn author(&self) -> &Author {
        &self.author
    }

    pub(crate) fn into_parts(self) -> (BookId, BookTitle, Author, S) {
        (self.id, self.title, self.author, self.state)
    }
}

// ANCHOR_END: book_typestate

// ANCHOR: stored_book
/// DB から復元した動的な状態を、状態ごとの本として保持します。
#[derive(Debug, Clone)]
pub(crate) enum StoredBook {
    WantToRead(Book<WantToRead>),
    Reading(Book<Reading>),
    Finished(Book<Finished>),
}

impl StoredBook {
    /// 保存値の検証後にだけ使う復元境界です。新規作成の遷移とは区別します。
    pub(crate) fn restore(
        id: BookId,
        title: BookTitle,
        author: Author,
        status: ReadingStatus,
    ) -> Self {
        match status {
            ReadingStatus::WantToRead => Self::WantToRead(Book::new(id, title, author)),
            ReadingStatus::Reading => Self::Reading(Book {
                id,
                title,
                author,
                state: Reading,
            }),
            ReadingStatus::Finished => Self::Finished(Book {
                id,
                title,
                author,
                state: Finished,
            }),
        }
    }

    pub(crate) fn id(&self) -> BookId {
        match self {
            Self::WantToRead(book) => book.id(),
            Self::Reading(book) => book.id(),
            Self::Finished(book) => book.id(),
        }
    }

    pub(crate) fn status(&self) -> ReadingStatus {
        match self {
            Self::WantToRead(_) => ReadingStatus::WantToRead,
            Self::Reading(_) => ReadingStatus::Reading,
            Self::Finished(_) => ReadingStatus::Finished,
        }
    }

    pub(crate) fn into_parts(self) -> (BookId, BookTitle, Author, ReadingStatus) {
        let status = self.status();
        let (id, title, author) = match self {
            Self::WantToRead(book) => {
                let (id, title, author, _) = book.into_parts();
                (id, title, author)
            }
            Self::Reading(book) => {
                let (id, title, author, _) = book.into_parts();
                (id, title, author)
            }
            Self::Finished(book) => {
                let (id, title, author, _) = book.into_parts();
                (id, title, author)
            }
        };
        (id, title, author, status)
    }
}

// ANCHOR_END: stored_book

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Author, BookId, BookTitle};

    #[test]
    fn transitions_preserve_book_data() {
        let book = Book::new(
            BookId(7),
            BookTitle::try_from("Rust Book".to_owned()).unwrap(),
            Author::try_from("Author".to_owned()).unwrap(),
        );
        let reading: Book<Reading> = book.start_reading();
        let finished: Book<Finished> = reading.finish();
        assert_eq!(finished.id(), BookId(7));
        assert_eq!(finished.title().as_str(), "Rust Book");
        assert_eq!(finished.author().as_str(), "Author");
    }
}
