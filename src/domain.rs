//! HTTP や SQLite の表現から独立した、アプリケーションの中心的な型です。

use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(transparent)]
// i64 のまま扱わず、将来 NoteId と取り違えたときにコンパイラが検出できるようにする。
pub(crate) struct BookId(pub(crate) i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(crate) struct NoteId(pub(crate) i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReadingStatus {
    WantToRead,
    Reading,
    Finished,
}

impl ReadingStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::WantToRead => "want_to_read",
            Self::Reading => "reading",
            Self::Finished => "finished",
        }
    }

    pub(crate) fn parse_input(value: &str) -> Result<Self, AppError> {
        match value {
            "want_to_read" => Ok(Self::WantToRead),
            "reading" => Ok(Self::Reading),
            "finished" => Ok(Self::Finished),
            _ => Err(AppError::Validation(
                "status must be one of: want_to_read, reading, finished".to_owned(),
            )),
        }
    }
}

impl TryFrom<&str> for ReadingStatus {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // DB は任意の文字列を返せるが、ドメイン層へ不正な状態を持ち込ませない。
        match value {
            "want_to_read" => Ok(Self::WantToRead),
            "reading" => Ok(Self::Reading),
            "finished" => Ok(Self::Finished),
            value => Err(AppError::InvalidStoredValue(format!(
                "unknown reading status: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Book {
    pub(crate) id: BookId,
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) status: ReadingStatus,
}

#[derive(Debug, Clone)]
pub(crate) struct Note {
    pub(crate) id: NoteId,
    pub(crate) body: String,
}

#[derive(Debug, Clone)]
pub(crate) struct BookDetail {
    pub(crate) book: Book,
    pub(crate) notes: Vec<Note>,
}
