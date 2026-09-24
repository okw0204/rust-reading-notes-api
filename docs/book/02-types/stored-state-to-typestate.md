# 保存状態を型状態へ接続する

## 問い

DB を読むまで状態が分からない本を、どうやって `Book<Reading>` に絞り、`finish()` を呼べるのでしょうか。型が防ぐ不正と、実行時に残る判断を分けます。

## 前提

第 1 部で、HTTP の本文が検証済みの `NoteBody` を所有するまでを確認しているものとします。

## 読む場所と順序

1. `src/domain/text.rs` の値型と `TryFrom<String>`。
2. `src/repository/sqlite.rs` の `BookRow` と `TryFrom<BookRow> for StoredBook`。
3. `src/domain/book.rs` の `Book<S>`、`StoredBook`、`restore`、`finish`。
4. `src/service.rs` の一冊を処理する `record_reading_completion`。
5. `src/domain/book.rs` の doctest。

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:book_row_conversion}}
```

```rust,ignore
{{#include ../../../src/domain/book.rs:stored_book}}
```

## 解説

SQLite の行は書名、著者、状態を文字列として返します。`TryFrom<BookRow>` は書名と著者を値型へ変換し、状態を `ReadingStatus` へ変換します。不正な保存値は利用者の入力不正ではなく `InvalidStoredValue` です。

実行時に得た状態は型引数 `S` へ直接代入できません。`StoredBook` は `WantToRead`、`Reading`、`Finished` の各 variant に異なる `Book<S>` を入れ、返り値を一つの enum にします。

service は次の形で読書中だけを取り出します。

```text
current = repository.find_book(book_id)
match current
  Reading(book) => book.finish()
  otherwise     => conflict
```

`StoredBook::Reading(book)` の枝では `book` の型が `Book<Reading>` と分かるため、`impl Book<Reading>` にだけ定義された `finish(self)` を呼べます。未読や読了の variant は実行時に `conflict` です。

`finish(self)` は本を消費して `Book<Finished>` を返します。同じ値に状態名を上書きするのではありません。未読の `Book<WantToRead>` には `finish` がないため、直接読了させるコードはコンパイル時に拒否されます。

ただし、型状態が保証するのは手元の所有値に対する合法な遷移です。同じ DB 行を別に取得した値や、取得後に変わった保存状態までは消せません。repository は保存時に現在状態が `reading` か再検査します。

## 別案との比較

### 状態を最後まで文字列で扱う

型は減りますが、未知の値や未読からの直接読了を各呼び出しで繰り返し検査する必要があります。境界で enum と型状態へ変換し、以後に許す操作を絞ります。

### `restore` を公開して任意の状態を作る

DB から検証済みの値を復元する crate 内操作としては必要です。しかし外部から自由に呼べると、新規作成時の遷移規則を迂回できます。現在の可視性と呼び出し場所を一緒に読みます。

### 型状態だけで保存競合も防ぐ

同じ ID の別スナップショットを型だけで一意にできないため成立しません。保存先の現在状態は条件付き更新で検査します。

## 確認

1. DB の状態が実行時まで不明でも、`Reading` の枝で `finish()` を呼べるのはなぜですか。
2. `Book<Reading>` は DB が現在も読書中であることを保証しますか。
3. 未知の HTTP 入力と未知の DB 保存値は、なぜ別の失敗分類ですか。
4. `finish(self)` の `self` 消費は何を保証しますか。

## 解答

1. `StoredBook` を match すると、variant の中身が `Book<Reading>` という具体型に絞られるためです。
2. 保証しません。取得後の変更があり得るため、保存時に実行時検査が必要です。
3. 前者は利用者が直せる入力規則、後者は信頼できるドメイン型へ復元できない内部状態だからです。
4. その所有値を遷移後の別の型へ移し、同じ元の値を再利用できなくすることです。別取得や clone の存在までは防ぎません。

次は[状態変更とメモを一緒に保存する](atomic-completion.md)で、型状態だけでは足りない保証を transaction がどう補うか読みます。
