# 保存状態を型状態へ接続する

## 問い

DB を読むまで状態が分からない本を、どうやって `Book<Reading>` に絞り、`finish()` を呼べるのでしょうか。型が防ぐ不正と、実行時に残る判断を分けます。

## 前提

第 1 部で、HTTP の本文が検証済みの `NoteBody` を所有するまでを確認しているものとします。

## 読む場所と順序

次の順で読みます。各節の直前に、対応する実コードの抜粋があります。

1. [DB 行を状態ごとの本へ復元する](#db-行を状態ごとの本へ復元する): SQLite の行を検証済みの値へ移す。
2. [状態を型引数に持つ Book](#状態を型引数に持つ-book): 3 つのマーカー型と、状態ごとにだけ生えるメソッド。
3. [実行時の状態を一つの enum に集める](#実行時の状態を一つの-enum-に集める): 実行時に分かる状態を状態別の本へ振り分ける。
4. [読書中の枝だけが finish を呼べる](#読書中の枝だけが-finish-を呼べる): service が一冊を処理する流れ。
5. [未知の入力と未知の保存値を分ける](#未知の入力と未知の保存値を分ける): 同じ enum でも入口ごとに失敗分類が違う理由。

## DB 行を状態ごとの本へ復元する

対象: `src/repository/sqlite.rs` の `BookRow` と `TryFrom<BookRow> for StoredBook`

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:book_row_struct}}
```

`#[derive(FromRow)]` により、SQLx は `SELECT` が返した列を `BookRow` のフィールドへ名前で割り当てます。`id` は `i64`、`title`、`author`、`status` は `String` です。SQLite の行はまだ検証されておらず、状態も文字列のままです。

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:book_row_conversion}}
```

`TryFrom<BookRow>` の処理を順に追います。

1. 行を所有権ごと受け取り、最後に `Ok(Self::restore(...))` で `StoredBook` を組み立てます。
2. `BookTitle::try_from(row.title)` と `Author::try_from(row.author)` は `String` を消費し、検証済みの値型へ移します。
3. その失敗は `.map_err(|error| AppError::InvalidStoredValue(error.to_string()))?` で包みます。壊れた保存値は利用者の入力不正ではなく、ドメイン型へ戻せない内部状態です。
4. `ReadingStatus::try_from(row.status.as_str())?` は `status` を借りた `&str` で状態 enum へ変換します。未知の値はここでも内部エラーになります。
5. 変換に使った `String` は移動または借用されるだけで、行全体をあとから使い回しません。

`TryFrom` の `type Error = AppError` があるため、`?` は `InvalidStoredValue` を含む `AppError` をそのまま呼び出し元へ返せます。

## 状態を型引数に持つ Book

対象: `src/domain/book.rs` の `Book<S>` と `WantToRead`、`Reading`、`Finished`

```rust,ignore
{{#include ../../../src/domain/book.rs:book_typestate}}
```

1. `WantToRead`、`Reading`、`Finished` は値を持たないマーカー型です。`Book<S>` の `state: S` にこのどれかが入ります。
2. `Book<WantToRead>::new` は新規作成専用で、`state` に `WantToRead` を入れます。未読の本しか作れません。
3. `start_reading(self)` は `self` を消費し、`id`・`title`・`author` を新しい値へ移し替えて `state: Reading` の `Book<Reading>` を返します。元の `Book<WantToRead>` はもう使えません。
4. `finish` は `impl Book<Reading>` にだけ定義されています。`Book<Reading>` を受け取り、`state: Finished` の `Book<Finished>` を返します。
5. `id`・`title`・`author` の取得は `impl<S> Book<S>` にあり、どの状態でも使えます。`into_parts` は crate 内専用です。

冒頭の doctest は `Book::new(...).start_reading().finish()` を実行し、状態を飛ばせないことを示します。

続く `compile_fail,E0599` は、未読の本へ直接 `finish()` を呼ぶコードがコンパイルに失敗することを固定します。未読を表す型には `finish` がないためです。`E0599` はメソッドが見つからないときのエラー番号です。

## 実行時の状態を一つの enum に集める

対象: `src/domain/book.rs` の `StoredBook` と `restore`

```rust
{{#include ../../../src/domain/book.rs:stored_book}}
```

1. `StoredBook` は variant ごとに別の `Book<S>` を持ちます。DB の状態は読むまで分からないため、型引数へ直接 `S` を書けません。
2. `restore` は検証済みの `id`・`title`・`author` と `ReadingStatus` を受け、`match status` で variant を選びます。
3. `WantToRead` の枝は `Book::new` を使い、`Reading` と `Finished` の枝は `state` を直接埋めて `Book` を組み立てます。フィールドは private ですが、同じモジュール内なので直接作れます。
4. `restore` は public ではありません。DB の値を検証したあとの復元境界でだけ呼び、新規作成の遷移規則を迂回できないようにしています。
5. `status()` は variant から `ReadingStatus` を返し、`into_parts` は本を消費して 4 つ組へ分解します。HTTP 応答や SQL のバインドで使います。

状態を型へ格上げするには、実行時の `match` が要ります。`restore` はその一回だけの入口です。

## 読書中の枝だけが finish を呼べる

対象: `src/service.rs` の `ReadingService::record_reading_completion`

```rust
{{#include ../../../src/service.rs:reading_completion_match}}
```

1. `find_book` で `StoredBook` を取得します。この時点では状態は実行時に決まります。
2. `match current` の `StoredBook::Reading(book)` の枝だけが `book` を取り出します。この枝では `book` の型が `Book<Reading>` と分かります。
3. ほかの variant は `_ => return Err(AppError::Conflict)` です。未読や読了は実行時の競合として扱います。
4. `reading.finish()` は `Book<Reading>` を消費して `Book<Finished>` を返します。
5. その `Book<Finished>` と `body: &NoteBody` を repository の `record_reading_completion` へ移し、保存された本とメモを `CompletedReading` で返します。

`StoredBook::Reading(book)` の枝では `book` の型が確定し、`impl Book<Reading>` にだけある `finish` を呼べます。

型が保証するのは、手元の所有値に対する合法な遷移です。同じ DB 行を別に取得した値や、取得後に変わった保存状態までは保証しません。repository は保存時に現在状態を再検査します。詳しくは[状態変更とメモを一緒に保存する](atomic-completion.md)で扱います。

## 未知の入力と未知の保存値を分ける

対象: `src/domain.rs` の `ReadingStatus`

```rust
{{#include ../../../src/domain.rs:reading_status}}
```

1. `ReadingStatus` は `WantToRead`・`Reading`・`Finished` の 3 値だけを持つ enum です。
2. `as_str` は DB へ書く/照合する `"want_to_read"` などの表現を返します。
3. `parse_input` は HTTP 入力を解釈し、未知の文字列なら `AppError::Validation` を返します。利用者が直せる入力規則の違反は `400 Bad Request` です。
4. `TryFrom<&str>` は DB の保存値を解釈し、未知なら `AppError::InvalidStoredValue` を返します。ドメイン型へ戻せない内部状態は `500` です。

同じ enum でも、入口が変われば失敗分類が変わります。`parse_input` は利用者向けの入力、`TryFrom<&str>` は保存値の復元専用です。両者を分けることで、未知の HTTP 入力と不正な保存値を同じ扱いにする誤りを防ぎます。

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
