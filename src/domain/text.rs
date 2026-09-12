//! 前後の空白を除去し、空でないことを構築時に保証する文字列型です。

// ANCHOR: validated_title
/// 検証済みの書名です。内部の文字列は直接変更できません。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookTitle(String);

/// 検証済みの著者名です。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author(String);

/// 検証済みのメモ本文です。本文内の改行や空白は保持します。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteBody(String);

/// 空白の除去後に文字が残らなかったことを表します。
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct InvalidText(&'static str);

fn normalize(value: String, message: &'static str) -> Result<String, InvalidText> {
    let value = value.trim();
    if value.is_empty() {
        return Err(InvalidText(message));
    }
    Ok(value.to_owned())
}

impl TryFrom<String> for BookTitle {
    type Error = InvalidText;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        normalize(value, "title must not be empty").map(Self)
    }
}

impl BookTitle {
    /// 検証済みの文字列を借用します。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 値を消費し、内部の文字列の所有権を返します。
    pub fn into_inner(self) -> String {
        self.0
    }
}

// ANCHOR_END: validated_title

impl TryFrom<String> for Author {
    type Error = InvalidText;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        normalize(value, "author must not be empty").map(Self)
    }
}

impl Author {
    /// 検証済みの文字列を借用します。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 値を消費し、内部の文字列の所有権を返します。
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for NoteBody {
    type Error = InvalidText;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        normalize(value, "note body must not be empty").map(Self)
    }
}

impl NoteBody {
    /// 検証済みの文字列を借用します。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 値を消費し、内部の文字列の所有権を返します。
    pub fn into_inner(self) -> String {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_trimmed_and_cannot_be_blank() {
        let title = BookTitle::try_from("  Rust Book\n".to_owned()).unwrap();
        assert_eq!(title.as_str(), "Rust Book");
        assert_eq!(title.into_inner(), "Rust Book");
        for blank in ["", " \t", "\n\u{3000}"] {
            assert_eq!(
                BookTitle::try_from(blank.to_owned())
                    .unwrap_err()
                    .to_string(),
                "title must not be empty"
            );
        }
    }

    #[test]
    fn author_is_trimmed_and_cannot_be_blank() {
        let author = Author::try_from("\tMara Bos  ".to_owned()).unwrap();
        assert_eq!(author.as_str(), "Mara Bos");
        assert_eq!(author.into_inner(), "Mara Bos");
        for blank in ["", " \t", "\n\u{3000}"] {
            assert_eq!(
                Author::try_from(blank.to_owned()).unwrap_err().to_string(),
                "author must not be empty"
            );
        }
    }

    #[test]
    fn note_body_is_trimmed_and_cannot_be_blank() {
        let body = NoteBody::try_from("\n 所有権\n借用 \t".to_owned()).unwrap();
        assert_eq!(body.as_str(), "所有権\n借用");
        assert_eq!(body.into_inner(), "所有権\n借用");
        for blank in ["", " \t", "\n\u{3000}"] {
            assert_eq!(
                NoteBody::try_from(blank.to_owned())
                    .unwrap_err()
                    .to_string(),
                "note body must not be empty"
            );
        }
    }
}
